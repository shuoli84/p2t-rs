use crate::{AdvancingFront, Edges, Points, TriangleId, Triangles};

pub struct Context<'a> {
    pub points: &'a Points,
    pub edges: &'a Edges,
    pub triangles: &'a mut Triangles,
    pub advancing_front: &'a mut AdvancingFront,
    pub result: Vec<TriangleId>,

    // reusable legalize task queue to reduce alloc overhead
    pub(crate) legalize_task_queue: Vec<TriangleId>,
    // reusable legalize remap triangle ids to reduce alloc overhead
    pub(crate) legalize_remap_tids: Vec<TriangleId>,
    // reusable legalize triangle id queue
    pub(crate) triangle_id_queue: Vec<TriangleId>,
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
            result: Vec::with_capacity(points.len()),

            legalize_task_queue: Vec::with_capacity(32),
            legalize_remap_tids: Vec::with_capacity(32),
            triangle_id_queue: Vec::with_capacity(32),
        }
    }
}
