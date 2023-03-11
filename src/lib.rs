mod advancing_front;
mod context;
mod edge;
mod points;
mod shape;
mod triangles;
mod utils;

use std::ops::AddAssign;

use advancing_front::AdvancingFront;
use context::Context;
use edge::{Edges, EdgesBuilder};
use points::Points;
use rustc_hash::FxHashMap;
use shape::*;
use triangles::{TriangleId, Triangles};
use utils::{in_circle, orient_2d, Orientation};

use crate::utils::in_scan_area;

pub use points::PointId;

pub struct SweeperBuilder {
    edges_builder: EdgesBuilder,
    points: Points,
}

impl SweeperBuilder {
    pub fn new(polyline: Vec<Point>) -> Self {
        let mut points = Points::new(vec![]);

        let edges = parse_polyline(polyline, &mut points);

        Self {
            edges_builder: EdgesBuilder::new(edges),
            points,
        }
    }

    pub fn add_point(&mut self, point: Point) -> &mut Self {
        self.points.add_point(point);
        self
    }

    pub fn add_hole(&mut self, polyline: Vec<Point>) -> &mut Self {
        let edges = parse_polyline(polyline, &mut self.points);
        self.edges_builder.add_edges(edges);
        self
    }

    pub fn build(self) -> Sweeper {
        Sweeper {
            points: self.points,
            edges: self.edges_builder.build(),
            triangles: Triangles::new(),
        }
    }
}

fn parse_polyline(polyline: Vec<Point>, points: &mut Points) -> Vec<Edge> {
    let mut edge_list = Vec::with_capacity(polyline.len());

    let mut point_iter = polyline.iter().map(|p| (points.add_point(*p), p));
    let first_point = point_iter.next().expect("empty polyline");

    let mut last_point = first_point;
    loop {
        match point_iter.next() {
            Some(p2) => {
                let edge = Edge::new(last_point, p2);
                edge_list.push(edge);
                last_point = p2;
            }
            None => {
                let edge = Edge::new(last_point, first_point);
                edge_list.push(edge);
                break;
            }
        }
    }

    edge_list
}

#[derive(Debug)]
pub struct Sweeper {
    points: Points,
    edges: Edges,
    triangles: Triangles,
}

impl Sweeper {
    pub fn add_point(&mut self, point: Point) -> PointId {
        self.points.add_point(point)
    }

    pub fn triangulate(&mut self) {
        // sort all points first
        self.points = std::mem::take(&mut self.points).into_sorted();

        let initial_triangle = self.triangles.insert(Triangle::new(
            self.points.get_id_by_y(0).unwrap(),
            Points::HEAD_ID,
            Points::TAIL_ID,
        ));

        // create the advancing front with initial triangle
        let mut advancing_front = AdvancingFront::new(
            self.triangles.get(initial_triangle).unwrap(),
            initial_triangle,
            &self.points,
        );

        let mut context = Context::new(
            &self.points,
            &self.edges,
            &mut self.triangles,
            &mut advancing_front,
        );

        Self::sweep_points(&mut context);
        context.messages.push("sweep done".into());
        context.draw();

        Self::finalize_polygon(&mut context);
        context.messages.push("finalize polygon".into());
        context.draw();

        dbg!(&context.statistic);
        assert!(Self::verify_result(&context));
    }

    fn sweep_points(context: &mut Context) {
        for (point_id, point) in context.points.iter_point_by_y(1) {
            Self::point_event(point_id, point, context);
            context.draw();

            for p in context.edges.p_for_q(point_id) {
                let edge = Edge { p: *p, q: point_id };
                Self::edge_event(edge, point, context);
                context.draw();
            }

            // if !Self::verify_triangles(context) {
            //     Self::fix_triangles(context);
            //     context.messages.push("fix invalid triangles".into());
            //     context.draw();
            // }
            assert!(Self::verify_triangles(context));
        }
    }

    fn finalize_polygon(context: &mut Context) -> Option<()> {
        // get an internal triangle to start with
        // the first node is head, artificial point, so skip
        let (_, node) = context.advancing_front.nth(1)?;

        let mut t = node.triangle?;

        loop {
            if let Some(tri) = context.triangles.get(t) {
                if !tri.constrained_edge_cw(node.point_id) {
                    t = tri.neighbor_ccw(node.point_id);
                } else {
                    break;
                }
            }
        }

        if !t.invalid() {
            Self::clean_mesh(t, context);
        }

        Some(())
    }

    fn clean_mesh(triangle_id: TriangleId, context: &mut Context) -> Option<()> {
        let mut triangles = Vec::<TriangleId>::new();
        triangles.push(triangle_id);

        while let Some(t) = triangles.pop() {
            if t.invalid() {
                continue;
            }

            let tri = context.triangles.get_mut(t).unwrap();

            if !tri.interior {
                tri.interior = true;
                context.result.push(t);

                for i in 0..3 {
                    if !tri.constrained_edge[i] {
                        triangles.push(tri.neighbors[i]);
                    }
                }
            }
        }

        Some(())
    }
}

// first need to visualize each step, then trouble shoot
// print detailed steps, like what changes this going to address.

/// Point event related methods
impl Sweeper {
    fn point_event(point_id: PointId, point: Point, context: &mut Context) {
        println!("\npoint event: {point_id:?} {point:?}");
        context
            .messages
            .push(format!("point event: {point_id:?} {point:?}"));

        let (node_point, node) = context.advancing_front.locate_node(point.x).unwrap();
        let (_, next_node) = context.advancing_front.next_node(node_point).unwrap();

        let triangle =
            context
                .triangles
                .insert(Triangle::new(point_id, node.point_id, next_node.point_id));
        let node_triangle = node.triangle.unwrap();
        context.triangles.mark_neighbor(node_triangle, triangle);
        context.advancing_front.insert(point_id, point, triangle);

        Self::legalize(triangle, context, None);

        // in middle case, the node's x should be less than point'x
        // in left case, they are same.
        if point.x <= node_point.x + f64::EPSILON {
            Self::fill_one(node_point, context);
        }

        Self::fill_advancing_front(point, context);
    }

    /// helper function to check wether triangle is legal
    fn is_legalize(triangle_id: TriangleId, context: &Context) -> bool {
        for point_idx in 0..3 {
            let triangle = context.triangles.get_unchecked(triangle_id);
            let opposite_triangle_id = triangle.neighbors[point_idx];
            let Some(opposite_triangle) = context.triangles.get(opposite_triangle_id) else {
                continue;
            };

            let p = triangle.points[point_idx];
            let op = opposite_triangle.opposite_point(&triangle, p);
            let oi = opposite_triangle.point_index(op).unwrap();

            if opposite_triangle.constrained_edge[oi] {
                continue;
            }

            let inside = unsafe {
                in_circle(
                    context.points.get_point_uncheck(p),
                    context.points.get_point_uncheck(triangle.point_ccw(p)),
                    context.points.get_point_uncheck(triangle.point_cw(p)),
                    context.points.get_point_uncheck(op),
                )
            };

            if inside {
                return false;
            }
        }

        true
    }

    /// legalize the triangle, but keep the edge index
    fn legalize(triangle_id: TriangleId, context: &mut Context, keep_edge_index: Option<usize>) {
        println!(
            "legalizing {} with {keep_edge_index:?}",
            triangle_id.as_usize()
        );

        let start_triangle_id = triangle_id;
        let mut triangle_tasks = FxHashMap::default();

        let mut task_queue = Vec::<TriangleId>::new();
        task_queue.push(triangle_id);
        triangle_tasks.insert(triangle_id, 1);

        while let Some(triangle_id) = task_queue.pop() {
            let mut f = || {
                println!("legalizing {triangle_id:?}");
                context.count_legalize_incr();

                for point_idx in 0..3 {
                    if triangle_id == start_triangle_id
                        && keep_edge_index.map(|i| i == point_idx).unwrap_or_default()
                    {
                        continue;
                    }

                    let triangle = context.triangles.get_unchecked(triangle_id);

                    let opposite_triangle_id = triangle.neighbors[point_idx];
                    let Some(opposite_triangle) = context.triangles.get(opposite_triangle_id) else {
                        continue;
                    };

                    let p = triangle.points[point_idx];
                    let op = opposite_triangle.opposite_point(&triangle, p);
                    let oi = opposite_triangle.point_index(op).unwrap();

                    // if this is a constrained edge or a delaunay edge(only during recursive legalization)
                    // then we should not try to legalize
                    if opposite_triangle.constrained_edge[oi] {
                        context.triangles.set_constrained(
                            triangle_id,
                            point_idx,
                            opposite_triangle.constrained_edge[oi],
                        );
                        continue;
                    }

                    let illegal = unsafe {
                        in_circle(
                            context.points.get_point_uncheck(p),
                            context.points.get_point_uncheck(triangle.point_ccw(p)),
                            context.points.get_point_uncheck(triangle.point_cw(p)),
                            context.points.get_point_uncheck(op),
                        )
                    };
                    if illegal {
                        println!(
                            "rotating: {} {} from:{:?} {:?}",
                            triangle_id.as_usize(),
                            opposite_triangle_id.as_usize(),
                            triangle_id.get(&context.triangles),
                            opposite_triangle_id.get(&context.triangles),
                        );
                        // rotate shared edge one vertex cw to legalize it
                        Self::rotate_triangle_pair(
                            triangle_id,
                            p,
                            opposite_triangle_id,
                            op,
                            context.triangles,
                        );

                        println!(
                            "  after: {} {} after:{:?} {:?}",
                            triangle_id.as_usize(),
                            opposite_triangle_id.as_usize(),
                            triangle_id.get(&context.triangles),
                            opposite_triangle_id.get(&context.triangles),
                        );
                        task_queue.push(triangle_id);
                        triangle_tasks.get_mut(&triangle_id).unwrap().add_assign(1);

                        task_queue.push(opposite_triangle_id);
                        triangle_tasks
                            .entry(opposite_triangle_id)
                            .or_insert(0)
                            .add_assign(1);
                    }
                }
            };
            f();
            let task_count = triangle_tasks.get_mut(&triangle_id).unwrap();
            *task_count -= 1;

            if *task_count == 0 {
                println!("triangle {} done", triangle_id.as_usize());
                if triangle_id.as_usize() == 123 {
                    println!("break here");
                }
                Self::map_triangle_to_nodes(triangle_id, context);
            }
        }
    }

    fn rotate_triangle_pair(
        triangle_id: TriangleId,
        p: PointId,
        ot_id: TriangleId,
        op: PointId,
        triangles: &mut Triangles,
    ) {
        let t = triangles.get(triangle_id).unwrap();
        let ot = triangles.get(ot_id).unwrap();

        let n1 = t.neighbor_ccw(p);
        let n2 = t.neighbor_cw(p);
        let n3 = ot.neighbor_ccw(op);
        let n4 = ot.neighbor_cw(op);

        let ce1 = t.constrained_edge_ccw(p);
        let ce2 = t.constrained_edge_cw(p);
        let ce3 = ot.constrained_edge_ccw(op);
        let ce4 = ot.constrained_edge_cw(op);

        // rotate shared edge one vertex cw to legalize it
        let t = triangles.get_mut_unchecked(triangle_id);
        t.rotate_cw(p, op);
        t.set_constrained_edge_cw(p, ce2);
        t.set_constrained_edge_ccw(op, ce3);
        t.clear_neighbors();

        let ot = triangles.get_mut_unchecked(ot_id);
        ot.rotate_cw(op, p);
        ot.set_constrained_edge_ccw(p, ce1);
        ot.set_constrained_edge_cw(op, ce4);
        ot.clear_neighbors();

        if !n2.invalid() {
            triangles.mark_neighbor(triangle_id, n2);
        }
        if !n3.invalid() {
            triangles.mark_neighbor(triangle_id, n3);
        }
        if !n1.invalid() {
            triangles.mark_neighbor(ot_id, n1);
        }
        if !n4.invalid() {
            triangles.mark_neighbor(ot_id, n4);
        }

        triangles.mark_neighbor(triangle_id, ot_id);
    }

    /// update advancing front node's triangle
    fn map_triangle_to_nodes(triangle_id: TriangleId, context: &mut Context) {
        let triangle = context.triangles.get(triangle_id).unwrap();
        for i in 0..3 {
            if triangle.neighbors[i].invalid() {
                let point = context
                    .points
                    .get_point(triangle.point_cw(triangle.points[i]))
                    .expect("should exist");
                if let Some(node) = context.advancing_front.locate_point_mut(point) {
                    node.triangle = Some(triangle_id);
                }
            }
        }
    }

    /// fill the node with one triangle.
    /// Note: The moment it filled, advancing_front is modified.
    /// if the node is covered by another triangle, then it is deleted from advancing_front.
    /// all following advancing front lookup is affected.
    fn fill_one(node_point: Point, context: &mut Context) -> Option<()> {
        let node = context.advancing_front.get_node(node_point).unwrap();
        let prev_node = context.advancing_front.prev_node(node_point)?;
        let next_node = context.advancing_front.next_node(node_point)?;

        let triangle_id = context.triangles.insert(Triangle::new(
            prev_node.1.point_id,
            node.point_id,
            next_node.1.point_id,
        ));

        if let Some(prev_tri) = prev_node.1.triangle {
            context.triangles.mark_neighbor(triangle_id, prev_tri);
        }
        if let Some(node_tri) = node.triangle {
            context.triangles.mark_neighbor(triangle_id, node_tri);
        }

        // update prev_node's triangle to newly created
        context
            .advancing_front
            .insert(prev_node.1.point_id, prev_node.0, triangle_id);

        println!(
            "create tri: {} {:?} nei: {:?} {:?} {:?}",
            triangle_id.as_usize(),
            triangle_id.get(&context.triangles),
            triangle_id.get(&context.triangles).neighbors[0].try_get(&context.triangles),
            triangle_id.get(&context.triangles).neighbors[1].try_get(&context.triangles),
            triangle_id.get(&context.triangles).neighbors[2].try_get(&context.triangles),
        );

        Self::legalize(triangle_id, context, None);

        // this node maybe shadowed by new triangle, delete it from advancing front
        let node = context.advancing_front.get_node(node_point).unwrap();
        let tri = context.triangles.get_unchecked(node.triangle.unwrap());
        if tri.point_index(node.point_id).is_none() || !tri.neighbor_cw(node.point_id).invalid() {
            // todo: we need to ensure all frontint node's triangle is updated. which means
            //     even for node needs to delete
            // 1.
            // if the node's triangle doesn't contain node, which
            // means the legalize process rotated the tri, and mapping
            // logic didn't get a chance to fix it. Then this node
            // is not a valid front node.
            // 2.
            // node's triangle has a valid neighbor, which means the edge is not on the front
            println!("deleting {node_point:?} from advancing front");
            context.advancing_front.delete(node_point);
        }
        Some(())
    }

    fn fill_advancing_front(node_point: Point, context: &mut Context) {
        {
            // fill right holes
            let mut node_point = node_point;
            while let Some((next_node_point, _)) = context.advancing_front.next_node(node_point) {
                if context.advancing_front.next_node(next_node_point).is_some() {
                    // if HoleAngle exceeds 90 degrees then break
                    if Self::large_hole_dont_fill(next_node_point, &context.advancing_front) {
                        break;
                    }

                    Self::fill_one(next_node_point, context);
                    node_point = next_node_point;
                } else {
                    break;
                }
            }
        }

        {
            // fill left holes
            let mut node_point = node_point;

            while let Some((prev_node_point, _)) = context.advancing_front.prev_node(node_point) {
                if context.advancing_front.prev_node(prev_node_point).is_some() {
                    // if HoleAngle exceeds 90 degrees then break
                    if Self::large_hole_dont_fill(prev_node_point, &context.advancing_front) {
                        break;
                    }

                    Self::fill_one(prev_node_point, context);
                    node_point = prev_node_point;
                } else {
                    break;
                }
            }
        }

        // file right basins
        if let Some(basin_angle) = Self::basin_angle(node_point, context.advancing_front) {
            if basin_angle < std::f64::consts::FRAC_PI_4 * 3. {
                Self::fill_basin(node_point, context);
            }
        }
    }

    fn large_hole_dont_fill(node_point: Point, advancing_front: &AdvancingFront) -> bool {
        let (next_point, _next_node) = advancing_front.next_node(node_point).unwrap();
        let (prev_point, _prev_node) = advancing_front.prev_node(node_point).unwrap();

        let angle = utils::Angle::new(node_point, next_point, prev_point);
        if angle.exceeds_90_degree() {
            return false;
        }
        if angle.is_negative() {
            return true;
        }

        // the original implentation also add two new check, which is not stated in the paper.
        // I just leave it later, will try it when have deeper understanding.

        true
    }
}

#[derive(Debug)]
struct ConstrainedEdge {
    constrained_edge: Edge,
    p: Point,
    q: Point,
    /// Whether the constrained edge is "right" edge, p.x larger than q.x
    right: bool,
}

impl ConstrainedEdge {
    fn p_id(&self) -> PointId {
        self.constrained_edge.p
    }

    fn q_id(&self) -> PointId {
        self.constrained_edge.q
    }

    fn with_q(&self, q: PointId, context: &Context) -> Self {
        let q_point = q.get(&context.points);
        Self {
            constrained_edge: Edge {
                p: self.constrained_edge.p,
                q,
            },
            p: self.p,
            q: q_point,
            right: self.p.x > q_point.x,
        }
    }
}

/// EdgeEvent related methods
impl Sweeper {
    fn edge_event(edge: Edge, node_point: Point, context: &mut Context) {
        println!("\nedge event: {edge:?}");

        context.messages.push(format!(
            "edge_event: p:{} q:{} node:{:?}",
            edge.p.as_usize(),
            edge.q.as_usize(),
            node_point
        ));

        let p = context.points.get_point(edge.p).unwrap();
        let q = context.points.get_point(edge.q).unwrap();

        let constrain_edge = ConstrainedEdge {
            constrained_edge: edge,
            p,
            q,
            right: p.x > q.x,
        };

        {
            // check and fill
            let node = context.advancing_front.get_node(node_point).unwrap();

            let triangle = node
                .triangle
                .expect("only af's last node has None triangle id");
            if Self::try_mark_edge_for_triangle(edge.p, edge.q, triangle, context) {
                // the edge is already an edge of the triangle, return
                context
                    .messages
                    .push("the edge is already an edge of the triangle, return".to_string());
                return;
            }

            // for now we will do all needed filling
            Self::fill_edge_event(&constrain_edge, node_point, context);
        }

        // node's triangle may changed, get the latest
        let triangle = context
            .advancing_front
            .get_node(node_point)
            .unwrap()
            .triangle
            .expect("only af's last node has None triangle id");

        // this triangle crosses constraint so let's flippin start!
        let mut triangle_ids = Vec::<TriangleId>::new();
        Self::edge_event_process(
            edge.p,
            edge.q,
            &constrain_edge,
            triangle,
            edge.q,
            &mut triangle_ids,
            context,
        );

        for triangle_id in triangle_ids {
            Self::legalize(triangle_id, context, None);
        }
    }

    /// try mark edge for triangle if the constrained edge already is a edge
    /// returns `true` if yes, otherwise `false`
    fn try_mark_edge_for_triangle(
        p: PointId,
        q: PointId,
        t_id: TriangleId,
        context: &mut Context,
    ) -> bool {
        let triangles = &mut context.triangles;
        let triangle = triangles.get_mut_unchecked(t_id);
        match triangle.edge_index(p, q) {
            None => {
                return false;
            }
            Some(index) => {
                triangle.constrained_edge[index] = true;

                // The triangle may or may not has a valid neighbor
                let neighbor_t_id = triangle.neighbors[index];
                if let Some(t) = triangles.get_mut(neighbor_t_id) {
                    let index = t.edge_index(p, q).unwrap();
                    t.constrained_edge[index] = true;
                }

                true
            }
        }
    }

    fn fill_edge_event(edge: &ConstrainedEdge, node_point: Point, context: &mut Context) {
        if edge.right {
            Self::fill_right_above_edge_event(edge, node_point, context);
        } else {
            Self::fill_left_above_edge_event(edge, node_point, context);
        }
    }

    fn fill_right_above_edge_event(
        edge: &ConstrainedEdge,
        mut node_point: Point,
        context: &mut Context,
    ) {
        context.messages.push(format!(
            "fill_right_above_edge_event: p:{} q:{}",
            edge.p_id().as_usize(),
            edge.q_id().as_usize(),
        ));

        while let Some((next_node_point, _)) = context.advancing_front.next_node(node_point) {
            if next_node_point.x >= edge.p.x {
                break;
            }

            // check if next node is below the edge
            if orient_2d(edge.p, next_node_point, edge.q).is_ccw() {
                Self::fill_right_below_edge_event(edge, node_point, context);
            } else {
                // try next node
                node_point = next_node_point;
            }
        }
    }

    fn fill_right_below_edge_event(
        edge: &ConstrainedEdge,
        node_point: Point,
        context: &mut Context,
    ) {
        if node_point.x >= edge.p.x {
            return;
        }

        let (next_node_point, _) = context.advancing_front.next_node(node_point).unwrap();
        let (next_next_node_point, _) = context.advancing_front.next_node(next_node_point).unwrap();

        if orient_2d(node_point, next_node_point, next_next_node_point).is_ccw() {
            // concave
            Self::fill_right_concave_edge_event(edge, node_point, context);
        } else {
            // convex
            Self::fill_right_convex_edge_event(edge, node_point, context);

            // retry this one
            Self::fill_right_below_edge_event(edge, node_point, context);
        }
    }

    /// recursively fill concave nodes
    fn fill_right_concave_edge_event(
        edge: &ConstrainedEdge,
        node_point: Point,
        context: &mut Context,
    ) {
        let (node_next_point, next_node) = context.advancing_front.next_node(node_point).unwrap();
        let next_node_point_id = next_node.point_id;
        Self::fill_one(node_next_point, context);

        if next_node_point_id != edge.p_id() {
            // next above or below edge?
            if orient_2d(edge.q, node_next_point, edge.p).is_ccw() {
                //  below
                let next_next_point = context
                    .advancing_front
                    .next_node(node_next_point)
                    .unwrap()
                    .0;
                if orient_2d(node_point, node_next_point, next_next_point).is_ccw() {
                    // next is concave
                    Self::fill_right_concave_edge_event(edge, node_point, context);
                } else {
                    // next is convex
                }
            }
        }
    }

    fn fill_right_convex_edge_event(
        edge: &ConstrainedEdge,
        node_point: Point,
        context: &mut Context,
    ) {
        let (next_node_point, _) = context.advancing_front.next_node(node_point).unwrap();
        let (next_next_node_point, _) = context.advancing_front.next_node(next_node_point).unwrap();
        let (next_next_next_node_point, _) = context
            .advancing_front
            .next_node(next_next_node_point)
            .unwrap();
        // next concave or convex?
        if orient_2d(
            next_node_point,
            next_next_node_point,
            next_next_next_node_point,
        )
        .is_ccw()
        {
            // concave
            Self::fill_right_concave_edge_event(edge, node_point, context);
        } else {
            // convex
            // next above or below edge?
            if orient_2d(edge.q, next_next_node_point, edge.p).is_ccw() {
                // Below
                Self::fill_right_convex_edge_event(edge, next_node_point, context);
            } else {
                // Above
            }
        }
    }

    fn fill_left_above_edge_event(
        edge: &ConstrainedEdge,
        mut node_point: Point,
        context: &mut Context,
    ) {
        while let Some((prev_node_point, _)) = context.advancing_front.prev_node(node_point) {
            // check if next node is below the edge
            if prev_node_point.x <= edge.p.x {
                break;
            }

            if orient_2d(edge.q, prev_node_point, edge.p).is_cw() {
                Self::fill_left_below_edge_event(edge, node_point, context);
            } else {
                node_point = prev_node_point;
            }
        }
    }

    fn fill_left_below_edge_event(
        edge: &ConstrainedEdge,
        node_point: Point,
        context: &mut Context,
    ) {
        if node_point.x > edge.p.x {
            let (prev_node_point, _) = context.advancing_front.prev_node(node_point).unwrap();
            let (prev_prev_node_point, _) =
                context.advancing_front.prev_node(prev_node_point).unwrap();
            if orient_2d(node_point, prev_node_point, prev_prev_node_point).is_cw() {
                Self::fill_left_concave_edge_event(edge, node_point, context);
            } else {
                // convex
                Self::fill_left_convex_edge_event(edge, node_point, context);

                // retry this one
                Self::fill_left_below_edge_event(edge, node_point, context);
            }
        }
    }

    fn fill_left_convex_edge_event(
        edge: &ConstrainedEdge,
        node_point: Point,
        context: &mut Context,
    ) {
        // next concave or convex?
        let (prev_node_point, _) = context.advancing_front.prev_node(node_point).unwrap();
        let (prev_prev_node_point, _) = context.advancing_front.prev_node(prev_node_point).unwrap();
        let (prev_prev_prev_node_point, _) = context
            .advancing_front
            .prev_node(prev_prev_node_point)
            .unwrap();

        if orient_2d(
            prev_node_point,
            prev_prev_node_point,
            prev_prev_prev_node_point,
        )
        .is_cw()
        {
            // concave
            Self::fill_left_concave_edge_event(edge, prev_node_point, context);
        } else {
            // convex
            // next above or below edge?
            if orient_2d(edge.q, prev_prev_node_point, edge.p).is_cw() {
                // below
                Self::fill_left_convex_edge_event(edge, prev_node_point, context);
            } else {
                // above
            }
        }
    }

    fn fill_left_concave_edge_event(
        edge: &ConstrainedEdge,
        node_point: Point,
        context: &mut Context,
    ) {
        let (prev_node_point, _) = context.advancing_front.prev_node(node_point).unwrap();
        Self::fill_one(prev_node_point, context);

        let (prev_node_point, prev_node) = context.advancing_front.prev_node(node_point).unwrap();

        if prev_node.point_id != edge.p_id() {
            // next above or below edge?
            if orient_2d(edge.q, prev_node_point, edge.p).is_cw() {
                // below
                let (prev_prev_node_point, _) =
                    context.advancing_front.prev_node(prev_node_point).unwrap();
                if orient_2d(node_point, prev_node_point, prev_prev_node_point).is_cw() {
                    // next is concave
                    Self::fill_left_concave_edge_event(edge, node_point, context);
                } else {
                    // next is convex
                }
            }
        }
    }

    fn edge_event_process(
        ep: PointId,
        eq: PointId,
        constrain_edge: &ConstrainedEdge,
        triangle_id: TriangleId,
        p: PointId,
        triangle_ids: &mut Vec<TriangleId>,
        context: &mut Context,
    ) {
        assert!(!triangle_id.invalid());

        if Self::try_mark_edge_for_triangle(ep, eq, triangle_id, context) {
            return;
        }

        let triangle = context.triangles.get_mut_unchecked(triangle_id);
        let p1 = triangle.point_ccw(p);
        let o1 = orient_2d(
            eq.get(&context.points),
            p1.get(&context.points),
            ep.get(&context.points),
        );

        if o1.is_collinear() {
            if let Some(edge_index) = triangle.edge_index(eq, p1) {
                triangle.constrained_edge[edge_index] = true;

                let neighbor_across_t = triangle.neighbor_across(p);
                Self::edge_event_process(
                    ep,
                    p1,
                    &constrain_edge.with_q(p1, context),
                    neighbor_across_t,
                    p1,
                    triangle_ids,
                    context,
                );
                return;
            } else {
                panic!("EdgeEvent - collinear points not supported")
            }
        }

        let p2 = triangle.point_cw(p);
        let o2 = orient_2d(
            eq.get(&context.points),
            p2.get(&context.points),
            ep.get(&context.points),
        );
        if o2.is_collinear() {
            if let Some(edge_index) = triangle.edge_index(eq, p2) {
                triangle.constrained_edge[edge_index] = true;

                let neighbor_across_t = triangle.neighbor_across(p);
                Self::edge_event_process(
                    ep,
                    p2,
                    &constrain_edge.with_q(p2, context),
                    neighbor_across_t,
                    p2,
                    triangle_ids,
                    context,
                );

                return;
            } else {
                panic!("collinear points not supported");
            }
        }

        if o1 == o2 {
            // need to decide if we are rotating cw or ccw to get to a triangle
            // that will cross edge
            let triangle_id = if o1.is_cw() {
                triangle.neighbor_ccw(p)
            } else {
                triangle.neighbor_cw(p)
            };

            Self::edge_event_process(
                ep,
                eq,
                constrain_edge,
                triangle_id,
                p,
                triangle_ids,
                context,
            );
        } else {
            Self::flip_edge_event(
                ep,
                eq,
                constrain_edge,
                triangle_id,
                p,
                triangle_ids,
                context,
            );
        }
    }
}

/// flip edge related methods
impl Sweeper {
    fn flip_edge_event(
        ep: PointId,
        eq: PointId,
        edge: &ConstrainedEdge,
        triangle_id: TriangleId,
        p: PointId,
        triangle_ids: &mut Vec<TriangleId>,
        context: &mut Context,
    ) {
        assert!(!triangle_id.invalid());

        let t = context.triangles.get_unchecked(triangle_id);

        println!(
            "getting neighbor_across tri:{} p:{}",
            triangle_id.as_usize(),
            p.as_usize()
        );
        let ot_id = t.neighbor_across(p);
        if ot_id.invalid() {
            println!("invalid neighbor: {} {t:?}", triangle_id.as_usize());
        }
        assert!(!ot_id.invalid(), "neighbor must be valid");

        let ot = context.triangles.get_unchecked(ot_id);

        let op = ot.opposite_point(t, p);
        if in_scan_area(
            p.get(&context.points),
            t.point_ccw(p).get(&context.points),
            t.point_cw(p).get(&context.points),
            op.get(&context.points),
        ) {
            // lets rotate shared edge one vertex cw
            Self::rotate_triangle_pair(triangle_id, p, ot_id, op, &mut context.triangles);
            Self::map_triangle_to_nodes(triangle_id, context);
            Self::map_triangle_to_nodes(ot_id, context);
            // legalize later
            triangle_ids.extend([triangle_id, ot_id]);

            if p == eq && op == ep {
                if eq == edge.q_id() && ep == edge.p_id() {
                    context
                        .triangles
                        .get_mut_unchecked(triangle_id)
                        .set_constrained_for_edge(ep, eq);

                    context
                        .triangles
                        .get_mut_unchecked(ot_id)
                        .set_constrained_for_edge(ep, eq);
                } else {
                    // original comment: I think one of the triangles should be legalized here?
                    // todo: figure this out
                }
            } else {
                let o = orient_2d(
                    eq.get(&context.points),
                    op.get(&context.points),
                    ep.get(&context.points),
                );

                let t = Self::next_flip_triangle(o, triangle_id, ot_id, triangle_ids);
                Self::flip_edge_event(ep, eq, edge, t, p, triangle_ids, context);
            }
        } else {
            let new_p = Self::next_flip_point(ep, eq, ot_id, op, context);
            Self::flip_scan_edge_event(
                ep,
                eq,
                edge,
                triangle_id,
                ot_id,
                new_p,
                triangle_ids,
                context,
            );
            Self::edge_event_process(ep, eq, edge, triangle_id, p, triangle_ids, context);
        }
    }

    fn next_flip_triangle(
        o: Orientation,
        t: TriangleId,
        ot: TriangleId,
        triangle_ids: &mut Vec<TriangleId>,
    ) -> TriangleId {
        if o.is_ccw() {
            // ot is not crossing edge after flip
            triangle_ids.push(ot);
            t
        } else {
            // t is not crossing edge after flip
            triangle_ids.push(t);
            ot
        }
    }

    fn next_flip_point(
        ep: PointId,
        eq: PointId,
        ot: TriangleId,
        op: PointId,
        context: &mut Context,
    ) -> PointId {
        let o2d = orient_2d(
            eq.get(&context.points),
            op.get(&context.points),
            ep.get(&context.points),
        );

        let ot = context.triangles.get_unchecked(ot);
        match o2d {
            Orientation::CW => {
                // right
                ot.point_ccw(op)
            }
            Orientation::CCW => {
                // left
                ot.point_cw(op)
            }
            Orientation::Collinear => {
                panic!("Opposing point on constrained edge");
            }
        }
    }

    fn flip_scan_edge_event(
        ep: PointId,
        eq: PointId,
        edge: &ConstrainedEdge,
        flip_triangle_id: TriangleId,
        t_id: TriangleId,
        p: PointId,
        triangle_ids: &mut Vec<TriangleId>,
        context: &mut Context,
    ) {
        let t = t_id.get(&context.triangles);
        let ot = t.neighbor_across(p);
        if ot.invalid() {
            panic!("flip_scan_edge_event - null neighbor across");
        }

        let op = ot.get(&context.triangles).opposite_point(t, p);
        let flip_triangle = flip_triangle_id.get(&context.triangles);
        let p1 = flip_triangle.point_ccw(eq);
        let p2 = flip_triangle.point_cw(eq);

        if in_scan_area(
            eq.get(&context.points),
            p1.get(&context.points),
            p2.get(&context.points),
            op.get(&context.points),
        ) {
            // flip with new edge op -> eq
            Self::flip_edge_event(eq, op, edge, ot, op, triangle_ids, context);

            // original comment:
            // TODO: Actually I just figured out that it should be possible to
            //       improve this by getting the next ot and op before the the above
            //       flip and continue the flipScanEdgeEvent here
            // set new ot and op here and loop back to inScanArea test
            // also need to set a new flip_triangle first
            // Turns out at first glance that this is somewhat complicated
            // so it will have to wait.
        } else {
            let new_p = Self::next_flip_point(ep, eq, ot, op, context);
            Self::flip_scan_edge_event(
                ep,
                eq,
                edge,
                flip_triangle_id,
                ot,
                new_p,
                triangle_ids,
                context,
            );
        }
    }
}

#[derive(Debug)]
struct Basin {
    left: Point,
    right: Point,
    bottom: Point,
    width: f64,
    left_higher: bool,
}

impl Basin {
    pub fn is_shallow(&self, point: Point) -> bool {
        let height = if self.left_higher {
            self.left.y - point.y
        } else {
            self.right.y - point.y
        };

        self.width > height
    }

    pub fn completed(&self, point: Point) -> bool {
        if point.x >= self.right.x || point.x <= self.left.x {
            return true;
        }

        self.is_shallow(point)
    }
}

/// Basin related methods
impl Sweeper {
    fn basin_angle(node_point: Point, advancing_front: &AdvancingFront) -> Option<f64> {
        let (next_point, _) = advancing_front.next_node(node_point)?;
        let (next_next_point, _) = advancing_front.next_node(next_point)?;

        let ax = node_point.x - next_next_point.x;
        let ay = node_point.y - next_next_point.y;
        Some(ay.atan2(ax))
    }

    /// basin is like a bowl, we first identify it's left, bottom, right node.
    /// then fill it
    fn fill_basin(node_point: Point, context: &mut Context) -> Option<()> {
        let next_node = context.advancing_front.next_node(node_point)?;
        let next_next_node = context.advancing_front.next_node(next_node.0)?;

        // find the left
        let left: Point;
        if orient_2d(node_point, next_node.0, next_next_node.0).is_ccw() {
            left = next_next_node.0;
        } else {
            left = next_node.0;
        }

        // find the bottom
        let mut bottom: Point = left;
        while let Some((next_node_point, _)) = context.advancing_front.next_node(bottom) {
            if bottom.y >= next_node_point.y {
                bottom = next_node_point;
            } else {
                break;
            }
        }

        // no valid basin
        if bottom.eq(&left) {
            return None;
        }

        // find the right
        let mut right = bottom;
        while let Some((next_node_point, _)) = context.advancing_front.next_node(right) {
            if right.y < next_node_point.y {
                right = next_node_point;
            } else {
                break;
            }
        }
        if right.eq(&bottom) {
            // no valid basin
            return None;
        }

        let width = right.x - left.x;
        let left_higher: bool = left.y > right.y;

        Self::fill_basin_req(
            bottom,
            &Basin {
                left,
                right,
                bottom,
                width,
                left_higher,
            },
            context,
        );

        Some(())
    }

    fn fill_basin_req(node: Point, basin: &Basin, context: &mut Context) -> Option<()> {
        if basin.completed(node) {
            return None;
        }

        println!("filling basin req {node:?} basin: {basin:?}");

        Self::fill_one(node, context);

        // find the next node to fill
        let prev_point = context.advancing_front.prev_node(node)?.0;
        let next_point = context.advancing_front.next_node(node)?.0;

        if prev_point.eq(&basin.left) && next_point.eq(&basin.right) {
            return Some(());
        }

        let new_node = if prev_point.eq(&basin.left) {
            let next_next_point = context.advancing_front.next_node(next_point)?.0;
            if orient_2d(node, next_point, next_next_point).is_cw() {
                return None;
            }

            next_point
        } else if next_point.eq(&basin.right) {
            let prev_prev_point = context.advancing_front.prev_node(prev_point)?.0;
            if orient_2d(node, prev_point, prev_prev_point).is_ccw() {
                return None;
            }

            prev_point
        } else {
            // continue with the neighbor node with lowest Y value
            if prev_point.y < next_point.y {
                prev_point
            } else {
                next_point
            }
        };

        Self::fill_basin_req(new_node, basin, context)
    }
}

impl Sweeper {
    fn verify_triangles(context: &Context) -> bool {
        let triangle_ids = context
            .triangles
            .iter()
            .map(|(t_id, _)| t_id)
            .collect::<Vec<_>>();

        let mut result = true;

        for t_id in triangle_ids {
            if !Self::is_legalize(t_id, context) {
                println!("{} not legal", t_id.as_usize());
                result = false;
            }
        }

        result
    }

    fn fix_triangles(context: &mut Context) {
        let triangle_ids = context
            .triangles
            .iter()
            .map(|(t_id, _)| t_id)
            .collect::<Vec<_>>();

        for t_id in triangle_ids {
            if !Self::is_legalize(t_id, context) {
                println!("{} not legal, fix it", t_id.as_usize());
                Sweeper::legalize(t_id, context, None);
            }
        }
    }

    fn verify_result(context: &Context) -> bool {
        let mut verify_result = true;
        for t_id in &context.result {
            if !Self::is_legalize(*t_id, context) {
                println!("{} not legal", t_id.as_usize());
                verify_result = false;
            }
        }

        verify_result
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use rand::Rng;

    use super::*;

    #[test]
    fn test_context() {
        let polyline = vec![
            Point::new(0., 0.),
            Point::new(200., 0.),
            Point::new(100., 400.),
            Point::new(0., 400.),
            Point::new(30., 300.),
            Point::new(140., 110.),
        ];
        let builder = SweeperBuilder::new(polyline);
        let mut sweeper = builder.build();
        sweeper.triangulate();
    }

    #[test]
    fn test_forever_rand() {
        let mut idx = 0;
        loop {
            idx += 1;
            println!("run {idx}");
            test_rand();
        }
    }

    #[test]
    fn test_rand() {
        // attach_debugger();
        let file_path = "test_data/lastest_test_data";

        let points = if let Some(points) = try_load_from_file(file_path) {
            points
        } else {
            let mut points = Vec::<Point>::new();
            for _ in 0..100 {
                let x: f64 = rand::thread_rng().gen_range(0.0..800.);
                let y: f64 = rand::thread_rng().gen_range(0.0..800.);
                points.push(Point::new(x, y));
            }
            save_to_file(&points, file_path);
            points
        };

        let mut builder = SweeperBuilder::new(vec![
            Point::new(-10., -10.),
            Point::new(810., -10.),
            Point::new(810., 810.),
            Point::new(-10., 810.),
        ]);
        for p in points {
            builder.add_point(p);
        }

        builder.add_hole(vec![
            Point::new(400., 400.),
            Point::new(600., 400.),
            Point::new(600., 600.),
            Point::new(400., 600.),
        ]);

        builder.build().triangulate();

        delete_file(file_path);
    }

    fn try_load_from_file(path: &str) -> Option<Vec<Point>> {
        let mut f = std::fs::File::options().read(true).open(path).ok()?;
        let mut value = "".to_string();
        f.read_to_string(&mut value).unwrap();
        let mut points = vec![];
        for line in value.lines() {
            let mut iter = line.split_whitespace();
            let x = iter.next().unwrap();
            let y = iter.next().unwrap();

            let x = x.parse::<f64>().unwrap();
            let y = y.parse::<f64>().unwrap();
            points.push(Point::new(x, y));
        }

        Some(points)
    }

    fn save_to_file(points: &[Point], path: &str) {
        use std::fmt::Write;

        let mut f = std::fs::File::options()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();

        let mut value = "".to_string();
        for p in points {
            writeln!(value, "{} {}", p.x, p.y).unwrap();
        }

        f.write_all(value.as_bytes()).unwrap();
    }

    fn delete_file(path: &str) {
        std::fs::remove_file(path).unwrap();
    }

    fn attach_debugger() {
        let url = format!(
            "vscode://vadimcn.vscode-lldb/launch/config?{{'request':'attach','pid':{}}}",
            std::process::id()
        );
        std::process::Command::new("code")
            .arg("--open-url")
            .arg(url)
            .output()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1)); // Wait for debugger to attach
    }
}
