//! Magnetic snapping of a dragged rectangle against the other panels.

use crate::metrics::Metrics;
use crate::rect::Rect;

/// Where `dragged` ends up when dropped near `others`, before the clamp to the window.
///
/// Each axis is solved on its own and picks the nearest candidate within the snap distance.
/// Candidates are docking (the edge goes to the other panel's opposite edge plus or minus the
/// gap, only when the panels overlap or nearly overlap on the other axis) and alignment (the
/// same edge of both on the same line). Docking wins a tie, so a corner is reached by docking
/// on one axis and aligning on the other.
pub(crate) fn snap(dragged: Rect, others: &[Rect], m: &Metrics) -> (f32, f32) {
    let x = snap_axis(
        Axis::of(dragged.x, dragged.w, dragged.y, dragged.h),
        others.iter().map(|o| Axis::of(o.x, o.w, o.y, o.h)),
        m,
    );
    let y = snap_axis(
        Axis::of(dragged.y, dragged.h, dragged.x, dragged.w),
        others.iter().map(|o| Axis::of(o.y, o.h, o.x, o.w)),
        m,
    );
    (x, y)
}

/// A rectangle seen along one axis: its start and length, then the start and length on the
/// other axis.
struct Axis {
    start: f32,
    len: f32,
    cross_start: f32,
    cross_len: f32,
}

impl Axis {
    fn of(start: f32, len: f32, cross_start: f32, cross_len: f32) -> Self {
        Axis {
            start,
            len,
            cross_start,
            cross_len,
        }
    }

    fn end(&self) -> f32 {
        self.start + self.len
    }
}

/// The snapped start of `dragged` along one axis.
fn snap_axis(dragged: Axis, others: impl Iterator<Item = Axis>, m: &Metrics) -> f32 {
    // The nearest candidate so far and its distance. The first one found wins a tie.
    let mut best: Option<(f32, f32)> = None;
    let mut consider = |candidate: f32| {
        let distance = (candidate - dragged.start).abs();
        if distance <= m.snap_distance && best.is_none_or(|(_, nearest)| distance < nearest) {
            best = Some((candidate, distance));
        }
    };
    let others: Vec<Axis> = others.collect();
    for other in &others {
        let near = dragged.cross_start < other.cross_start + other.cross_len + m.snap_distance
            && dragged.cross_start + dragged.cross_len > other.cross_start - m.snap_distance;
        if near {
            consider(other.end() + m.gap);
            consider(other.start - m.gap - dragged.len);
        }
    }
    for other in &others {
        consider(other.start);
        consider(other.end() - dragged.len);
    }
    best.map_or(dragged.start, |(candidate, _)| candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics() -> Metrics {
        Metrics::default()
    }

    fn target() -> Rect {
        Rect::new(100.0, 100.0, 200.0, 100.0)
    }

    #[test]
    fn docks_to_the_right_of_another_panel() {
        let dragged = Rect::new(310.0, 100.0, 50.0, 100.0);
        assert_eq!(snap(dragged, &[target()], &metrics()).0, 306.0);
    }

    #[test]
    fn docks_to_the_left_of_another_panel() {
        let dragged = Rect::new(52.0, 100.0, 50.0, 100.0);
        assert_eq!(snap(dragged, &[target()], &metrics()).0, 44.0);
    }

    #[test]
    fn docks_below_another_panel() {
        let dragged = Rect::new(100.0, 195.0, 200.0, 50.0);
        assert_eq!(snap(dragged, &[target()], &metrics()).1, 206.0);
    }

    #[test]
    fn docks_above_another_panel() {
        let dragged = Rect::new(100.0, 40.0, 200.0, 50.0);
        assert_eq!(snap(dragged, &[target()], &metrics()).1, 44.0);
    }

    #[test]
    fn aligns_lefts_rights_tops_and_bottoms() {
        let t = target();
        let narrow = |x, y| Rect::new(x, y, 50.0, 40.0);
        assert_eq!(snap(narrow(95.0, 500.0), &[t], &metrics()).0, 100.0);
        assert_eq!(snap(narrow(255.0, 500.0), &[t], &metrics()).0, 250.0);
        assert_eq!(snap(narrow(900.0, 95.0), &[t], &metrics()).1, 100.0);
        assert_eq!(snap(narrow(900.0, 165.0), &[t], &metrics()).1, 160.0);
    }

    #[test]
    fn corner_alignment_combines_docking_and_alignment() {
        // Docked to the right of the target and top aligned with it.
        let dragged = Rect::new(309.0, 104.0, 50.0, 40.0);
        assert_eq!(snap(dragged, &[target()], &metrics()), (306.0, 100.0));
        // Docked below the target and left aligned with it.
        let dragged = Rect::new(97.0, 203.0, 50.0, 40.0);
        assert_eq!(snap(dragged, &[target()], &metrics()), (100.0, 206.0));
    }

    #[test]
    fn nothing_snaps_beyond_the_distance() {
        let dragged = Rect::new(320.0, 100.0, 50.0, 100.0);
        assert_eq!(snap(dragged, &[target()], &metrics()).0, 320.0);
        let far = Rect::new(113.0, 230.0, 50.0, 40.0);
        assert_eq!(snap(far, &[target()], &metrics()), (113.0, 230.0));
    }

    #[test]
    fn docking_needs_the_panels_to_overlap_on_the_other_axis() {
        // Close in x to the target's right edge but far below it: no docking on x.
        let dragged = Rect::new(310.0, 400.0, 50.0, 100.0);
        assert_eq!(snap(dragged, &[target()], &metrics()).0, 310.0);
        // Nearly overlapping (within the snap distance) still docks.
        let dragged = Rect::new(310.0, 205.0, 50.0, 100.0);
        assert_eq!(snap(dragged, &[target()], &metrics()).0, 306.0);
    }

    #[test]
    fn picks_the_nearest_candidate() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(112.0, 0.0, 100.0, 100.0);
        // Left edge at 105: docking to a (106) is 1 away, aligning with b (112) is 7 away.
        let dragged = Rect::new(105.0, 0.0, 50.0, 100.0);
        assert_eq!(snap(dragged, &[a, b], &metrics()).0, 106.0);
    }

    #[test]
    fn docking_wins_a_tie_with_alignment() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(112.0, 0.0, 100.0, 100.0);
        // 109 is 3 from docking to a (106) and 3 from aligning with b (112).
        let dragged = Rect::new(109.0, 0.0, 50.0, 100.0);
        assert_eq!(snap(dragged, &[a, b], &metrics()).0, 106.0);
    }

    #[test]
    fn without_other_panels_nothing_moves() {
        let dragged = Rect::new(5.0, 7.0, 50.0, 100.0);
        assert_eq!(snap(dragged, &[], &metrics()), (5.0, 7.0));
    }
}
