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
    /// Create a new [`Edges`] from edges
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

    /// Get all `lower point p` [`PointId`] slice for q
    pub fn p_for_q(&self, q: PointId) -> &[PointId] {
        self.point_edges.get(&q).map(|edge_indexes| {
            &self.edge_lower_points[edge_indexes.start..edge_indexes.end]
        }).unwrap_or(&[])
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
    pub fn all_edges(&self) -> Vec<Edge> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edges() {
        let edges = Edges::new(vec![
            Edge {
                p: PointId(0),
                q: PointId(1),
            },

            Edge {
                p: PointId(1),
                q: PointId(2),
            },

            Edge {
                p: PointId(2),
                q: PointId(3),
            },

            Edge {
                p: PointId(0),
                q: PointId(3),
            },
        ]);

        assert_eq!(edges.all_edges().len(), 4);
        assert_eq!(edges.len(), 4);

        assert_eq!(edges.p_for_q(PointId(0)).len(), 0);
        assert_eq!(edges.p_for_q(PointId(1)).len(), 1);
        assert_eq!(edges.p_for_q(PointId(2)).len(), 1);
        assert_eq!(edges.p_for_q(PointId(3)).len(), 2);
    }
}