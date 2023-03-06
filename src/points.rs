use crate::{shape::Point, PointId};

/// Point store, provide a unique [`PointId`]
pub struct Points {
    points: Vec<Point>,
}

impl Points {
    pub fn new(points: Vec<Point>) -> Self {
        Self {
            points
        }
    }

    pub fn add_point(&mut self, point: Point) -> PointId {
        let point_id = PointId(self.points.len());
        self.points.push(point);
        point_id
    }
}