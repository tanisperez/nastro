//! An axis-aligned rectangle in logical pixels.

/// A rectangle: top-left corner and size, in window coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Rect {
    /// A rectangle from its corner and size.
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }

    /// The x coordinate of the right edge.
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    /// The y coordinate of the bottom edge.
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    /// The smallest rectangle that contains both.
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(
            x,
            y,
            self.right().max(other.right()) - x,
            self.bottom().max(other.bottom()) - y,
        )
    }

    /// Whether the two share some area. Rectangles that only touch do not intersect.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// Whether the point is inside. The left and top edges belong to the rectangle, the right
    /// and bottom edges do not.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// A copy moved by `(dx, dy)`.
    pub fn translated(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges() {
        let r = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert_eq!(r.right(), 40.0);
        assert_eq!(r.bottom(), 60.0);
    }

    #[test]
    fn union_covers_both() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(20.0, 5.0, 10.0, 30.0);
        assert_eq!(a.union(&b), Rect::new(0.0, 0.0, 30.0, 35.0));
        assert_eq!(b.union(&a), a.union(&b));
    }

    #[test]
    fn touching_rectangles_do_not_intersect() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(a.intersects(&Rect::new(9.0, 9.0, 10.0, 10.0)));
        assert!(!a.intersects(&Rect::new(10.0, 0.0, 10.0, 10.0)));
        assert!(!a.intersects(&Rect::new(0.0, 10.0, 10.0, 10.0)));
        assert!(!a.intersects(&Rect::new(50.0, 50.0, 1.0, 1.0)));
    }

    #[test]
    fn contains_is_half_open() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains(0.0, 0.0));
        assert!(r.contains(9.9, 9.9));
        assert!(!r.contains(10.0, 5.0));
        assert!(!r.contains(5.0, 10.0));
        assert!(!r.contains(-0.1, 5.0));
    }

    #[test]
    fn translated_keeps_the_size() {
        assert_eq!(
            Rect::new(1.0, 2.0, 3.0, 4.0).translated(10.0, -1.0),
            Rect::new(11.0, 1.0, 3.0, 4.0)
        );
    }
}
