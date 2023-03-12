use crate::{AdvancingFront, Edges, Points, TriangleId, Triangles};

pub struct Context<'a> {
    pub points: &'a Points,
    pub edges: &'a Edges,
    pub triangles: &'a mut Triangles,
    pub advancing_front: &'a mut AdvancingFront,
    pub result: Vec<TriangleId>,

    // reuse legalize task queue to reduce alloc overhead
    pub(crate) legalize_task_queue: Vec<TriangleId>,
}

impl<'a> Context<'a> {
    pub fn new(
        points: &'a Points,
        edges: &'a Edges,
        triangles: &'a mut Triangles,
        advancing_front: &'a mut AdvancingFront,
    ) -> Self {
        Self {
            points,
            edges,
            triangles,
            advancing_front,
            result: Default::default(),

            legalize_task_queue: Default::default(),
        }
    }
}
