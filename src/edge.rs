use smallvec::{smallvec, SmallVec};

use crate::{Edge, PointId};

/// Builder for `Edges`
#[derive(Clone)]
pub struct EdgesBuilder {
    edges_list: Vec<Vec<Edge>>,
}

impl EdgesBuilder {
    pub fn new(edges: Vec<Edge>) -> Self {
        Self {
            edges_list: vec![edges],
        }
    }

    pub fn add_edges(&mut self, edges: Vec<Edge>) -> &mut Self {
        self.edges_list.push(edges);
        self
    }

    pub fn build(self, point_size: usize) -> Edges {
        let mut edges = Vec::with_capacity(self.edges_list.iter().map(|el| el.len()).sum());

        for edges_list_item in self.edges_list {
            edges.extend(edges_list_item.into_iter());
        }

        Edges::new(edges, point_size)
    }
}

/// A compact edge storage for edges. In this algorithm, only store
/// edge in higher end point, so we just need to store the `lower` point.
/// Each point may contains 0 or many edges, instead of create a vec
/// for each point, store all points info in one big vec, and each
/// point just stores start, end index. This reduce mem alloc dramatically
#[derive(Debug, Clone)]
pub struct Edges {
    point_edges: Vec<SmallVec<[PointId; 2]>>,
}

impl Edges {
    /// Create a new [`Edges`] from edges
    pub fn new(edges: Vec<Edge>, point_size: usize) -> Self {
        let mut point_edges = vec![smallvec![]; point_size];
        for edge in edges {
            point_edges[edge.q.as_usize()].push(edge.p);
        }

        Self { point_edges }
    }

    /// Get all `lower point p` [`PointId`] slice for q
    pub fn p_for_q(&self, q: PointId) -> &[PointId] {
        self.point_edges[q.as_usize()].as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edges() {
        let edges = Edges::new(
            vec![
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
            ],
            10,
        );

        assert_eq!(edges.p_for_q(PointId(0)).len(), 0);
        assert_eq!(edges.p_for_q(PointId(1)).len(), 1);
        assert_eq!(edges.p_for_q(PointId(2)).len(), 1);
        assert_eq!(edges.p_for_q(PointId(3)).len(), 2);
    }
}
