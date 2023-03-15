use std::cmp::Ordering;

use crate::shape::Point;

/// new type for point id, currently is the index in context
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PointId(pub(crate) usize);

impl PointId {
    /// Get the inner value as usize
    pub fn as_usize(&self) -> usize {
        self.0
    }

    /// helper method used in the crate when I know the `PointId` is valid in `Points`
    pub(crate) fn get(&self, points: &Points) -> Point {
        unsafe { points.get_point_uncheck(*self) }
    }
}

#[derive(Default, Clone)]
pub struct PointsBuilder {
    points: Vec<Point>,
}

impl PointsBuilder {
    /// Create a new builder
    pub fn new(points: Vec<Point>) -> Self {
        Self { points }
    }

    /// Add a point
    pub fn add_point(&mut self, point: Point) -> PointId {
        let point_id = PointId(self.points.len());
        self.points.push(point);
        point_id
    }

    /// Add all `points`
    pub fn add_points(&mut self, points: impl IntoIterator<Item = Point>) {
        self.points.extend(points);
    }

    pub fn build(self) -> Points {
        Points::new(self.points)
    }
}

/// Point store
#[derive(Debug, Clone)]
pub struct Points {
    points: Vec<Point>,
    y_sorted: Vec<PointId>,
    pub head: PointId,
    pub tail: PointId,
}

impl Points {
    pub fn new(mut points: Vec<Point>) -> Self {
        let mut unsorted_points = points
            .iter()
            .enumerate()
            .map(|(idx, p)| (PointId(idx), p))
            .collect::<Vec<_>>();

        // sort by y
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
            let mut xmax = points[0].x;
            let mut xmin = xmax;
            let mut ymax = points[0].y;
            let mut ymin = ymax;

            for point in points.iter() {
                xmax = xmax.max(point.x);
                xmin = xmin.min(point.x);
                ymax = ymax.max(point.y);
                ymin = ymin.min(point.y);
            }

            let dx = (xmax - xmin) * 0.3;
            let dy = (ymax - ymin) * 0.3;

            let head = Point::new(xmin - dx, ymin - dy);
            let tail = Point::new(xmax + dx, ymin - dy);
            let head_id = PointId(points.len());
            points.push(head);
            let tail_id = PointId(points.len());
            points.push(tail);
            (head_id, tail_id)
        };

        Self {
            points,
            y_sorted: sorted_ids,
            head,
            tail,
        }
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// get point for id
    #[inline(never)]
    pub fn get_point(&self, point_id: PointId) -> Option<Point> {
        self.points.get(point_id.0).cloned()
    }

    /// get point for id
    #[inline(never)]
    pub unsafe fn get_point_uncheck(&self, point_id: PointId) -> Point {
        unsafe { self.points.get_unchecked(point_id.0).clone() }
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
