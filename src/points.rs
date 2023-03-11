use std::cmp::Ordering;

use crate::shape::Point;

/// new type for point id, currently is the index in context
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PointId(pub(crate) usize);

impl PointId {
    pub fn get(&self, points: &Points) -> Point {
        unsafe { points.get_point_uncheck(*self) }
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }
}

/// Point store
#[derive(Debug, Default)]
pub struct Points {
    points: Vec<Point>,
    y_sorted: Vec<PointId>,
    pub head: Point,
    pub tail: Point,
}

impl Points {
    /// Head point is the head virtual point
    pub const HEAD_ID: PointId = PointId(usize::MAX);
    /// Head point is the tail virtual point
    pub const TAIL_ID: PointId = PointId(usize::MAX - 1);

    pub fn new(points: Vec<Point>) -> Self {
        Self {
            points,
            y_sorted: vec![],
            head: Default::default(),
            tail: Default::default(),
        }
    }

    /// only call this after all points/edge mutation done
    pub fn into_sorted(self) -> Self {
        let mut unsorted_points = self
            .points
            .iter()
            .enumerate()
            .map(|(idx, p)| (PointId(idx), p))
            .collect::<Vec<_>>();

        unsorted_points.sort_by(|p1, p2| {
            let p1 = p1.1;
            let p2 = p2.1;

            if p1.y < p2.y {
                Ordering::Less
            } else if p1.y == p2.y {
                if p1.x < p2.x {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            } else {
                Ordering::Greater
            }
        });
        let sorted_ids = unsorted_points
            .into_iter()
            .map(|(idx, _)| idx)
            .collect::<Vec<_>>();
        let (head, tail) = {
            let mut xmax = self.points[0].x;
            let mut xmin = xmax;
            let mut ymax = self.points[0].y;
            let mut ymin = ymax;

            for point in self.points.iter() {
                xmax = xmax.max(point.x);
                xmin = xmin.min(point.x);
                ymax = ymax.max(point.y);
                ymin = ymin.min(point.y);
            }

            let dx = (xmax - xmin) * 0.3;
            let dy = (ymax - ymin) * 0.3;

            let head = Point::new(xmin - dx, ymin - dy);
            let tail = Point::new(xmax + dx, ymin - dy);
            (head, tail)
        };

        Self {
            points: self.points,
            y_sorted: sorted_ids,
            head,
            tail,
        }
    }

    /// Add a point
    pub fn add_point(&mut self, point: Point) -> PointId {
        let point_id = PointId(self.points.len());
        self.points.push(point);
        point_id
    }

    /// Add all `points`
    pub fn add_points(
        &mut self,
        points: impl IntoIterator<Item = Point>,
    ) -> impl Iterator<Item = PointId> + 'static {
        let start_point = PointId(self.points.len());
        self.points.extend(points);
        let end_point = PointId(self.points.len());
        PointRangeIter::new(start_point, end_point)
    }

    /// get point for id
    pub fn get_point(&self, point_id: PointId) -> Option<Point> {
        if point_id == Self::HEAD_ID {
            Some(self.head)
        } else if point_id == Self::TAIL_ID {
            Some(self.tail)
        } else {
            self.points.get(point_id.0).cloned()
        }
    }

    /// get point for id
    pub unsafe fn get_point_uncheck(&self, point_id: PointId) -> Point {
        if point_id == Self::HEAD_ID {
            self.head
        } else if point_id == Self::TAIL_ID {
            self.tail
        } else {
            unsafe { self.points.get_unchecked(point_id.0).clone() }
        }
    }

    /// get point by y order
    pub fn get_point_by_y(&self, order: usize) -> Option<Point> {
        let id = self.y_sorted.get(order)?;
        Some(self.points[id.0])
    }

    pub fn iter_point_by_y<'a>(
        &'a self,
        order: usize,
    ) -> impl Iterator<Item = (PointId, Point)> + 'a {
        self.y_sorted.iter().skip(order).map(|id| {
            let point = self.points[id.0];
            (*id, point)
        })
    }

    /// get point by y order
    /// Not including head, tail
    pub fn get_id_by_y(&self, order: usize) -> Option<PointId> {
        self.y_sorted.get(order).cloned()
    }

    /// iter all points
    pub fn iter(&self) -> impl Iterator<Item = (PointId, &Point)> {
        self.points
            .iter()
            .enumerate()
            .map(|(idx, p)| (PointId(idx), p))
    }
}

struct PointRangeIter {
    start: PointId,
    end: PointId,
    next: PointId,
}

impl PointRangeIter {
    fn new(start: PointId, end: PointId) -> Self {
        Self {
            start,
            end,
            next: start,
        }
    }
}

impl Iterator for PointRangeIter {
    type Item = PointId;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.next;
        if next != self.end {
            self.next.0 += 1;
            Some(next)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_points() {
        let points = Points::new(vec![
            Point::new(1., 1.),
            Point::new(1., 2.),
            Point::new(1., 5.),
            Point::new(1., 3.),
        ]);

        let points = points.into_sorted();

        assert_eq!(points.get_point_by_y(0).unwrap().y, 1.);
        assert_eq!(points.get_point_by_y(1).unwrap().y, 2.);
        assert_eq!(points.get_point_by_y(2).unwrap().y, 3.);
        assert_eq!(points.get_point_by_y(3).unwrap().y, 5.);
    }
}
