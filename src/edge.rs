use crate::{PointId, Edge};

/// A compact edge storage for edges. In this algorithm, only store
/// edge in higher end point, so we just need to store the `lower` point.
/// Each point may contains 0 or many edges, instead of create a vec
/// for each point, store all points info in one big vec, and each
/// point just stores start, end index. This reduce mem alloc dramatically
#[derive(Default, Debug)]
pub struct Edges {
    edge_lower_points: Vec<PointId>,
    point_edges: rustc_hash::FxHashMap<PointId, PointEdges>,
}

impl Edges {
    pub fn new(mut edges: Vec<Edge>) -> Self {
        let mut store = Self::default();

        // sort edges by higher end
        edges.sort_by(|l, r| l.q.cmp(&r.q));

        for edge in edges {
            store.add_edge(edge);
        }

        store
    }

    fn add_edge(&mut self, edge: Edge) {
        let start_idx = self.edge_lower_points.len();
        self.edge_lower_points.push(edge.p);
        let end_idx = start_idx + 1;

        self.point_edges.entry(edge.q).and_modify(|e| {
            e.end = end_idx;
        }).or_insert(PointEdges {
            start: start_idx,
            end: end_idx,
        });
    }

    /// Returns number of edges
    pub fn len(&self) -> usize {
        self.edge_lower_points.len()
    }

    /// call `f` for each edge, order is not ganrenteed
    pub fn foreach_edge(&self, mut f: impl FnMut(Edge)) {
        self.point_edges.iter().for_each(|(q, edge_indexes)| {
            for index in edge_indexes.start..edge_indexes.end {
                // safety: the init logic ensures index is valid
                let p = unsafe {
                    self.edge_lower_points.get_unchecked(index)
                };

                let edge = Edge {
                    p: *p,
                    q: *q,
                };

                f(edge);
            }
        });
    }

    /// get all edges
    pub fn get_edges(&self) -> Vec<Edge> {
        let mut result = Vec::with_capacity(self.len());
        self.foreach_edge(|e| result.push(e));
        result
    }
}


#[derive(Debug, Copy, Clone)]
struct PointEdges {
    /// start index in [`Edges`]
    start: usize,
    /// end index in [`Edges`]
    end: usize,
}