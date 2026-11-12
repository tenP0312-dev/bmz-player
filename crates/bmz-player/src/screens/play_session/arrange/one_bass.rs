//! One-bass ("ワンバス") lane and seed calculation.
//!
//! With a plain RANDOM arrange, the player can pin the chart's leftmost key (the "bass"
//! lane of the source chart) to the key they hold. The result is an ordinary RANDOM
//! layout of an ordinary 24-bit seed, so scores, replays and IR payloads need no
//! one-bass specific data. This module only computes: which lane is requested and which
//! seed produces it. It reads no input and owns no state.
use super::*;
use crate::random_option_seed::{RANDOM_OPTION_SEED_MAX, RandomOptionSeed};

/// Fresh seeds drawn before falling back to an ordered scan of the whole seed range.
const FRESH_SEED_ATTEMPTS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OneBassSide {
    P1,
    P2,
}

impl OneBassSide {
    const fn arrange_side(self) -> ArrangeSide {
        match self {
            Self::P1 => ArrangeSide::P1,
            Self::P2 => ArrangeSide::P2,
        }
    }
}

/// Where the starting layout comes from.
#[derive(Debug, Clone, Copy)]
pub(crate) enum OneBassBase<'a> {
    /// A normal play. `current` is the seed already chosen for this side, if any.
    Own { current: Option<RandomOptionSeed> },
    /// A layout borrowed from another play (rival / replay) as a lane permutation
    /// (`permutation[source] = destination`). Only two lanes are swapped.
    Borrowed { permutation: &'a [usize] },
}

/// Key modes the operation applies to: 5K, 7K, 10K, 14K and 9K (PMS).
pub(crate) const fn supports_key_mode(key_mode: KeyMode) -> bool {
    matches!(key_mode, KeyMode::K5 | KeyMode::K7 | KeyMode::K9 | KeyMode::K10 | KeyMode::K14)
}

/// The side's RANDOM lane group, scratch excluded, in left-to-right order.
fn side_keys(key_mode: KeyMode, side: OneBassSide) -> Option<Vec<usize>> {
    if !supports_key_mode(key_mode) {
        return None;
    }
    let groups = arrange_lane_groups_for_side(key_mode, false, side.arrange_side());
    let keys = groups.into_iter().next()?;
    (keys.len() >= 2).then_some(keys)
}

/// The chart's leftmost key on this side: the lane the bass notes start from.
#[cfg(test)]
pub(crate) fn source_lane(key_mode: KeyMode, side: OneBassSide) -> Option<Lane> {
    side_keys(key_mode, side)?.first().map(|&lane| Lane::ALL[lane])
}

/// The lane to pin the bass to: `pressed` must hold exactly one of this side's keys.
/// The scratch and every lane of the other side are ignored.
pub(crate) fn pressed_target(
    key_mode: KeyMode,
    side: OneBassSide,
    pressed: impl IntoIterator<Item = Lane>,
) -> Option<Lane> {
    let keys = side_keys(key_mode, side)?;
    let mut held: Vec<Lane> =
        pressed.into_iter().filter(|lane| keys.contains(&lane.index())).collect();
    held.sort_unstable_by_key(|lane| lane.index());
    held.dedup();
    match held.as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}

/// The plain RANDOM permutation (`permutation[source] = destination`) this side gets from
/// `seed`. It is the same computation the arrange pipeline uses for a normal RANDOM.
pub(crate) fn seed_permutation(
    key_mode: KeyMode,
    side: OneBassSide,
    seed: RandomOptionSeed,
) -> Option<Vec<usize>> {
    side_keys(key_mode, side)?;
    let groups = arrange_lane_groups_for_side(key_mode, false, side.arrange_side());
    Some(shuffle_lane_groups(i64::from(seed.value()), groups, false))
}

/// The first 24-bit seed whose RANDOM layout for this side equals `permutation`.
pub(crate) fn seed_for_permutation(
    key_mode: KeyMode,
    side: OneBassSide,
    permutation: &[usize],
) -> Option<RandomOptionSeed> {
    let keys = side_keys(key_mode, side)?;
    if permutation.len() != LANE_COUNT {
        return None;
    }
    // The target must be a permutation of the side's keys that leaves the others alone.
    let mut destinations: Vec<usize> = keys.iter().map(|&lane| permutation[lane]).collect();
    destinations.sort_unstable();
    let mut sorted_keys = keys.clone();
    sorted_keys.sort_unstable();
    if destinations != sorted_keys {
        return None;
    }
    (0..=RANDOM_OPTION_SEED_MAX).find_map(|value| {
        let mut rng = ArrangeRng::new(i64::from(value), false);
        shuffle_lane_group_matches(&mut rng, &keys, permutation)
            .then(|| RandomOptionSeed::new(value))
            .flatten()
    })
}

fn seed_pins(
    key_mode: KeyMode,
    side: OneBassSide,
    source: usize,
    target: Lane,
    seed: RandomOptionSeed,
) -> bool {
    seed_permutation(key_mode, side, seed)
        .is_some_and(|permutation| permutation[source] == target.index())
}

/// The seed that puts the chart's leftmost key on `target`, or `None` when one-bass does
/// not apply or no seed is found (the caller then keeps its ordinary RANDOM seed).
///
/// `entropy` supplies fresh 24-bit seeds. A seed that already pins the bass is reused.
pub(crate) fn one_bass_seed(
    key_mode: KeyMode,
    side: OneBassSide,
    target: Lane,
    base: OneBassBase<'_>,
    entropy: &mut dyn FnMut() -> RandomOptionSeed,
) -> Option<RandomOptionSeed> {
    let keys = side_keys(key_mode, side)?;
    let source = keys[0];
    if !keys.contains(&target.index()) {
        return None;
    }
    match base {
        OneBassBase::Own { current } => {
            if let Some(current) = current
                && seed_pins(key_mode, side, source, target, current)
            {
                return Some(current);
            }
            fresh_pinning_seed(key_mode, side, source, target, entropy)
        }
        OneBassBase::Borrowed { permutation } => {
            if permutation.len() != LANE_COUNT {
                return None;
            }
            let mut swapped = permutation.to_vec();
            if swapped[source] != target.index() {
                // Exchange the two lanes: the bass takes the target, and whatever was on
                // the target goes where the bass used to be.
                let other = keys.iter().copied().find(|&lane| swapped[lane] == target.index())?;
                swapped.swap(source, other);
            }
            seed_for_permutation(key_mode, side, &swapped)
        }
    }
}

fn fresh_pinning_seed(
    key_mode: KeyMode,
    side: OneBassSide,
    source: usize,
    target: Lane,
    entropy: &mut dyn FnMut() -> RandomOptionSeed,
) -> Option<RandomOptionSeed> {
    for _ in 0..FRESH_SEED_ATTEMPTS {
        let candidate = entropy();
        if seed_pins(key_mode, side, source, target, candidate) {
            return Some(candidate);
        }
    }
    (0..=RANDOM_OPTION_SEED_MAX)
        .filter_map(RandomOptionSeed::new)
        .find(|&seed| seed_pins(key_mode, side, source, target, seed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(value: u32) -> RandomOptionSeed {
        RandomOptionSeed::new(value).unwrap()
    }

    /// A deterministic stand-in for the OS entropy source.
    fn counter(start: u32) -> impl FnMut() -> RandomOptionSeed {
        let mut next = start;
        move || {
            next = (next + 7_919) & RANDOM_OPTION_SEED_MAX;
            seed(next)
        }
    }

    fn keys(key_mode: KeyMode, side: OneBassSide) -> Vec<Lane> {
        side_keys(key_mode, side).unwrap().into_iter().map(|lane| Lane::ALL[lane]).collect()
    }

    fn assert_permutes_only(permutation: &[usize], group: &[Lane]) {
        let group: Vec<usize> = group.iter().map(|lane| lane.index()).collect();
        let mut destinations: Vec<usize> = group.iter().map(|&lane| permutation[lane]).collect();
        destinations.sort_unstable();
        let mut sorted = group.clone();
        sorted.sort_unstable();
        assert_eq!(destinations, sorted, "no lane is dropped or duplicated");
        for (lane, &destination) in permutation.iter().enumerate() {
            if !group.contains(&lane) {
                assert_eq!(destination, lane, "lane {lane} outside the group is fixed");
            }
        }
    }

    #[test]
    fn only_the_documented_key_modes_apply() {
        for mode in [KeyMode::K5, KeyMode::K7, KeyMode::K9, KeyMode::K10, KeyMode::K14] {
            assert!(supports_key_mode(mode), "{mode:?}");
            assert!(source_lane(mode, OneBassSide::P1).is_some(), "{mode:?}");
        }
        for mode in [KeyMode::K4, KeyMode::K6, KeyMode::K8] {
            assert!(!supports_key_mode(mode), "{mode:?}");
            assert_eq!(source_lane(mode, OneBassSide::P1), None, "{mode:?}");
        }
        // The second side exists only in double play.
        assert_eq!(source_lane(KeyMode::K7, OneBassSide::P2), None);
        assert_eq!(source_lane(KeyMode::K14, OneBassSide::P2), Some(Lane::Key8));
        assert_eq!(source_lane(KeyMode::K14, OneBassSide::P1), Some(Lane::Key1));
    }

    #[test]
    fn exactly_one_key_of_the_side_selects_the_target() {
        let mode = KeyMode::K7;
        let side = OneBassSide::P1;
        assert_eq!(pressed_target(mode, side, []), None);
        assert_eq!(pressed_target(mode, side, [Lane::Key4]), Some(Lane::Key4));
        assert_eq!(pressed_target(mode, side, [Lane::Key4, Lane::Key4]), Some(Lane::Key4));
        assert_eq!(pressed_target(mode, side, [Lane::Key4, Lane::Key6]), None);
        // The scratch is not counted.
        assert_eq!(pressed_target(mode, side, [Lane::Scratch]), None);
        assert_eq!(pressed_target(mode, side, [Lane::Key4, Lane::Scratch]), Some(Lane::Key4));
        assert_eq!(pressed_target(mode, side, [Lane::Key4, Lane::Key6, Lane::Scratch]), None);
    }

    #[test]
    fn double_play_judges_each_side_separately() {
        let mode = KeyMode::K14;
        let pressed = [Lane::Key3, Lane::Key10];
        assert_eq!(pressed_target(mode, OneBassSide::P1, pressed), Some(Lane::Key3));
        assert_eq!(pressed_target(mode, OneBassSide::P2, pressed), Some(Lane::Key10));
        let pressed = [Lane::Key3, Lane::Key10, Lane::Key12];
        assert_eq!(pressed_target(mode, OneBassSide::P1, pressed), Some(Lane::Key3));
        assert_eq!(pressed_target(mode, OneBassSide::P2, pressed), None);
        // Scratch2 belongs to the second side and is ignored there too.
        assert_eq!(
            pressed_target(mode, OneBassSide::P2, [Lane::Key10, Lane::Scratch2]),
            Some(Lane::Key10)
        );
    }

    #[test]
    fn every_target_gets_a_seed_that_pins_the_bass_with_a_valid_permutation() {
        for (mode, side) in [
            (KeyMode::K5, OneBassSide::P1),
            (KeyMode::K7, OneBassSide::P1),
            (KeyMode::K9, OneBassSide::P1),
            (KeyMode::K10, OneBassSide::P1),
            (KeyMode::K10, OneBassSide::P2),
            (KeyMode::K14, OneBassSide::P1),
            (KeyMode::K14, OneBassSide::P2),
        ] {
            let group = keys(mode, side);
            let source = group[0];
            for &target in &group {
                let mut entropy = counter(11);
                let found = one_bass_seed(
                    mode,
                    side,
                    target,
                    OneBassBase::Own { current: None },
                    &mut entropy,
                )
                .unwrap_or_else(|| panic!("{mode:?} {side:?} {target:?}"));
                assert!(found.value() <= RANDOM_OPTION_SEED_MAX);
                // The seed alone reproduces the layout through the ordinary RANDOM path.
                let permutation = seed_permutation(mode, side, found).unwrap();
                assert_eq!(permutation[source.index()], target.index());
                assert_permutes_only(&permutation, &group);
            }
        }
    }

    #[test]
    fn the_seed_matches_the_ordinary_random_arrange_of_the_whole_chart() {
        let mode = KeyMode::K7;
        let group = keys(mode, OneBassSide::P1);
        let found = one_bass_seed(
            mode,
            OneBassSide::P1,
            Lane::Key4,
            OneBassBase::Own { current: None },
            &mut counter(3),
        )
        .unwrap();
        // random_lane_permutation is what a normal RANDOM play applies for this seed.
        let normal = random_lane_permutation(i64::from(found.value()), mode, false, false);
        assert_eq!(normal[Lane::Key1.index()], Lane::Key4.index());
        assert_eq!(normal[Lane::Scratch.index()], Lane::Scratch.index());
        assert_permutes_only(&normal, &group);
    }

    #[test]
    fn a_current_seed_that_already_pins_the_bass_is_kept() {
        let mode = KeyMode::K7;
        let side = OneBassSide::P1;
        // Seed 0 is a valid seed like any other.
        for current in [0, 1, 4_752, 12_345] {
            let current = seed(current);
            let permutation = seed_permutation(mode, side, current).unwrap();
            let target = Lane::ALL[permutation[Lane::Key1.index()]];
            let kept = one_bass_seed(
                mode,
                side,
                target,
                OneBassBase::Own { current: Some(current) },
                &mut counter(0),
            );
            assert_eq!(kept, Some(current));
        }
        // When it does not, a new seed is chosen.
        let current = seed(4_752); // identity layout: the bass stays on Key1
        let moved = one_bass_seed(
            mode,
            side,
            Lane::Key5,
            OneBassBase::Own { current: Some(current) },
            &mut counter(0),
        )
        .unwrap();
        assert_ne!(moved, current);
        assert_eq!(
            seed_permutation(mode, side, moved).unwrap()[Lane::Key1.index()],
            Lane::Key5.index()
        );
    }

    #[test]
    fn a_target_outside_the_side_or_an_unsupported_mode_does_nothing() {
        let own = OneBassBase::Own { current: None };
        assert_eq!(
            one_bass_seed(KeyMode::K7, OneBassSide::P1, Lane::Key9, own, &mut counter(0)),
            None
        );
        assert_eq!(
            one_bass_seed(KeyMode::K7, OneBassSide::P1, Lane::Scratch, own, &mut counter(0)),
            None
        );
        assert_eq!(
            one_bass_seed(KeyMode::K8, OneBassSide::P1, Lane::Key3, own, &mut counter(0)),
            None
        );
        assert_eq!(
            one_bass_seed(KeyMode::K14, OneBassSide::P1, Lane::Key9, own, &mut counter(0)),
            None,
            "a 2P key cannot be the 1P bass"
        );
    }

    #[test]
    fn a_borrowed_layout_changes_at_most_two_lanes() {
        let mode = KeyMode::K7;
        let side = OneBassSide::P1;
        let group = keys(mode, side);
        for borrowed_seed in [0, 1, 4_752, 777_777, RANDOM_OPTION_SEED_MAX] {
            let borrowed = seed_permutation(mode, side, seed(borrowed_seed)).unwrap();
            for &target in &group {
                let found = one_bass_seed(
                    mode,
                    side,
                    target,
                    OneBassBase::Borrowed { permutation: &borrowed },
                    &mut counter(0),
                )
                .unwrap_or_else(|| panic!("borrowed {borrowed_seed} target {target:?}"));
                let result = seed_permutation(mode, side, found).unwrap();
                assert_eq!(result[Lane::Key1.index()], target.index());
                let differences = (0..LANE_COUNT).filter(|&i| result[i] != borrowed[i]).count();
                assert!(differences <= 2, "borrowed {borrowed_seed} target {target:?}");
                assert_permutes_only(&result, &group);
            }
        }
    }

    #[test]
    fn a_borrowed_layout_that_already_pins_the_bass_is_reproduced() {
        let mode = KeyMode::K7;
        let side = OneBassSide::P1;
        let borrowed = seed_permutation(mode, side, seed(31_337)).unwrap();
        let target = Lane::ALL[borrowed[Lane::Key1.index()]];
        let found = one_bass_seed(
            mode,
            side,
            target,
            OneBassBase::Borrowed { permutation: &borrowed },
            &mut counter(0),
        )
        .unwrap();
        assert_eq!(seed_permutation(mode, side, found).unwrap(), borrowed);
    }

    #[test]
    fn malformed_borrowed_layouts_are_rejected() {
        let mode = KeyMode::K7;
        let side = OneBassSide::P1;
        let short = vec![0usize; 3];
        assert_eq!(
            one_bass_seed(
                mode,
                side,
                Lane::Key3,
                OneBassBase::Borrowed { permutation: &short },
                &mut counter(0)
            ),
            None
        );
        let mut duplicated: Vec<usize> = (0..LANE_COUNT).collect();
        duplicated[Lane::Key2.index()] = Lane::Key1.index();
        assert_eq!(seed_for_permutation(mode, side, &duplicated), None);
    }

    /// Every ordering of the 5- and 7-lane groups has a 24-bit seed. For 7 lanes the first
    /// 5040 orderings all appear within the first 50,000 seeds.
    #[test]
    fn every_ordering_of_five_and_seven_lanes_has_a_seed() {
        for (mode, count, scan_to) in [(KeyMode::K5, 120, 5_000), (KeyMode::K7, 5_040, 50_000)] {
            let mut seen = std::collections::HashSet::new();
            for value in 0..=scan_to {
                seen.insert(seed_permutation(mode, OneBassSide::P1, seed(value)).unwrap());
            }
            assert_eq!(seen.len(), count, "{mode:?}");
        }
    }

    /// Slow: scans the whole seed range for a few 9-lane orderings (PMS) and prints the time.
    #[test]
    #[ignore = "measures the 9-lane (PMS) seed search; run with --ignored --nocapture"]
    fn nine_lane_orderings_have_a_seed_and_the_search_time_is_reported() {
        let mode = KeyMode::K9;
        let side = OneBassSide::P1;
        let group = keys(mode, side);
        let source = group[0].index();
        let started = std::time::Instant::now();
        let reversed: Vec<usize> = {
            let mut permutation: Vec<usize> = (0..LANE_COUNT).collect();
            for (index, lane) in group.iter().enumerate() {
                permutation[lane.index()] = group[group.len() - 1 - index].index();
            }
            permutation
        };
        let found = seed_for_permutation(mode, side, &reversed);
        println!("9-lane reversed ordering: {found:?} in {:?}", started.elapsed());
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(seed_permutation(mode, side, found).unwrap(), reversed);
        assert_eq!(reversed[source], group[group.len() - 1].index());
    }
}
