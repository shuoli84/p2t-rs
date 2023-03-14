use crate::advancing_front::AdvancingFront;
use crate::edge::{Edges, EdgesBuilder};
use crate::points::Points;
use crate::triangles::TriangleId;
use crate::triangles::Triangles;
use crate::utils::{in_circle, in_scan_area, orient_2d, Orientation};
use crate::{shape::*, Context, PointId, Triangle};

/// Observer for sweeper, used to monitor how sweeper works, quite useful
/// for visual debugging when things goes wrong. Check example's draw.
#[allow(unused_variables)]
pub trait Observer {
    /// A point_event processed
    fn point_event(&mut self, point_id: PointId, context: &Context) {}

    /// An edge event processed
    fn edge_event(&mut self, edge: Edge, context: &Context) {}

    /// Sweep process done
    fn sweep_done(&mut self, context: &Context) {}

    /// The result finalized, holes, fake points etc cleaned.
    fn finalized(&mut self, context: &Context) {}

    /// About to legalize for triangle
    #[inline]
    fn will_legalize(&mut self, triangle_id: TriangleId, context: &Context) {}

    /// A single step inside one legalization process
    #[inline]
    fn legalize_step(&mut self, triangle_id: TriangleId, context: &Context) {}

    /// The triangle legalized
    #[inline]
    fn legalized(&mut self, triangel_id: TriangleId, context: &Context) {}
}

/// Default dummy observer, blank impl, so all calls should be optimized out by compiler.
impl Observer for () {}

/// Sweeper Builder
///
/// # Example
/// ```rust
///    use poly2tri_rs::{SweeperBuilder, Point};
///
///    let builder = SweeperBuilder::new(vec![
///        Point::new(-10., -10.),
///        Point::new(810., -10.),
///        Point::new(810., 810.),
///        Point::new(-10., 810.),
///    ]).add_steiner_points(vec![
///        Point::new(50., 50.),
///    ]).add_hole(vec![
///        Point::new(400., 400.),
///        Point::new(600., 400.),
///        Point::new(600., 600.),
///        Point::new(400., 600.),
///    ]);
///    let sweeper = builder.build();
/// ```

pub struct SweeperBuilder {
    edges_builder: EdgesBuilder,
    points: Points,
}

impl SweeperBuilder {
    /// Create a new Builder with polyline
    /// There should be only one polyline, and multiple holes and steiner points supported
    pub fn new(polyline: Vec<Point>) -> Self {
        let mut points = Points::new(vec![]);

        let edges = parse_polyline(polyline, &mut points);

        Self {
            edges_builder: EdgesBuilder::new(edges),
            points,
        }
    }

    /// Add a single sparse `Point`, there is no edge attached to it
    /// NOTE: if the point locates outside of polyline, then it has no
    /// effect on the final result
    pub fn add_steiner_point(mut self, point: Point) -> Self {
        self.points.add_point(point);
        self
    }

    /// Add multiple [`Point`], batch version for `Self::add_point`
    pub fn add_steiner_points(mut self, points: impl IntoIterator<Item = Point>) -> Self {
        let _ = self.points.add_points(points);
        self
    }

    /// Add a hole defined by polyline.
    pub fn add_hole(mut self, polyline: Vec<Point>) -> Self {
        let edges = parse_polyline(polyline, &mut self.points);
        self.edges_builder.add_edges(edges);
        self
    }

    /// Add holes
    pub fn add_holes(mut self, holes: impl IntoIterator<Item = Vec<Point>>) -> Self {
        for polyline in holes.into_iter() {
            self = self.add_hole(polyline);
        }
        self
    }

    /// build the sweeper
    pub fn build(self) -> Sweeper {
        Sweeper {
            points: self.points.into_sorted(),
            edges: self.edges_builder.build(),
        }
    }
}

/// Main interface, user should grab a new Sweeper by [`SweeperBuilder::build`]
#[derive(Debug, Clone)]
pub struct Sweeper {
    points: Points,
    edges: Edges,
}

/// The result of triangulate
struct Trianglulate {
    /// points store, it includes all points, including ones in hole
    points: Points,
    /// including all triangles, including ones in hole
    triangles: Triangles,
    /// final result `TriangleId`s
    result: Vec<TriangleId>,

    /// iterator next cursor
    next: usize,
}

impl Iterator for Trianglulate {
    type Item = Triangle;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next < self.result.len() {
            let index = self.next;
            self.next += 1;

            // safety: just checked index less than len
            let tri_id = unsafe { self.result.get_unchecked(index) };
            let triangle = tri_id.get(&self.triangles);

            return Some(Triangle {
                points: [
                    triangle.points[0].get(&self.points),
                    triangle.points[1].get(&self.points),
                    triangle.points[2].get(&self.points),
                ],
            });
        } else {
            None
        }
    }
}

impl Sweeper {
    /// Run trianglate with dummy observer
    pub fn triangulate(self) -> impl Iterator<Item = Triangle> {
        self.triangulate_with_observer(&mut ())
    }

    /// Run triangulate with observer
    pub fn triangulate_with_observer(
        self,
        observer: &mut impl Observer,
    ) -> impl Iterator<Item = Triangle> {
        let mut triangles = Triangles::with_capacity(self.points.len());

        let initial_triangle = triangles.insert(InnerTriangle::new(
            self.points.get_id_by_y(0).unwrap(),
            self.points.head,
            self.points.tail,
        ));

        // create the advancing front with initial triangle
        let mut advancing_front = AdvancingFront::new(
            triangles.get(initial_triangle).unwrap(),
            initial_triangle,
            &self.points,
        );

        let mut context = Context::new(
            &self.points,
            &self.edges,
            &mut triangles,
            &mut advancing_front,
        );

        Self::sweep_points(&mut context, observer);
        observer.sweep_done(&context);

        Self::finalize_polygon(&mut context);
        observer.finalized(&context);

        // take result out of context
        let result = context.result;

        Trianglulate {
            points: self.points,
            triangles,
            result,

            next: 0,
        }
    }
}

impl Sweeper {
    fn sweep_points(context: &mut Context, observer: &mut impl Observer) {
        for (point_id, point) in context.points.iter_point_by_y(1) {
            Self::point_event(point_id, point, context, observer);
            observer.point_event(point_id, context);

            for p in context.edges.p_for_q(point_id) {
                let edge = Edge { p: *p, q: point_id };
                Self::edge_event(edge, point, context, observer);

                observer.edge_event(edge, context);
            }

            debug_assert!(Self::verify_triangles(context));
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
        // id and from, it should not trigger from again
        let mut triangles = Vec::<(TriangleId, TriangleId)>::with_capacity(context.points.len());
        triangles.push((triangle_id, TriangleId::INVALID));

        while let Some((t, from)) = triangles.pop() {
            if t.invalid() {
                continue;
            }

            let tri = context.triangles.get_mut(t).unwrap();

            if !tri.interior {
                tri.interior = true;
                context.result.push(t);

                for i in 0..3 {
                    if !tri.is_constrained(i) {
                        let new_t = tri.neighbors[i];
                        if new_t != from {
                            triangles.push((new_t, t));
                        }
                    }
                }
            }

            #[cfg(feature = "draw_detail")]
            context.draw();
        }

        Some(())
    }
}

// first need to visualize each step, then trouble shoot
// print detailed steps, like what changes this going to address.

/// Point event related methods
impl Sweeper {
    fn point_event(
        point_id: PointId,
        point: Point,
        context: &mut Context,
        observer: &mut impl Observer,
    ) {
        let (node, next) = context.advancing_front.locate_node_and_next(point.x);
        let (node_point, node) = node.unwrap();
        let (_, next_node) = next.unwrap();

        let triangle = context.triangles.insert(InnerTriangle::new(
            point_id,
            node.point_id,
            next_node.point_id,
        ));
        let node_triangle = node.triangle.unwrap();
        context.triangles.mark_neighbor(node_triangle, triangle);
        context.advancing_front.insert(point_id, point, triangle);

        Self::legalize(triangle, context, observer);

        // in middle case, the node's x should be less than point'x
        // in left case, they are same.
        if point.x <= node_point.x + f64::EPSILON {
            Self::fill_one(node_point, context, observer);
        }

        Self::fill_advancing_front(point, context, observer);
    }

    /// helper function to check wether triangle is legal
    fn is_legalize(triangle_id: TriangleId, context: &Context) -> [TriangleId; 3] {
        let mut result = [TriangleId::INVALID; 3];
        for point_idx in 0..3 {
            let triangle = context.triangles.get_unchecked(triangle_id);
            let opposite_triangle_id = triangle.neighbors[point_idx];
            let Some(opposite_triangle) = context.triangles.get(opposite_triangle_id) else {
                continue;
            };

            let p = triangle.points[point_idx];
            let op = opposite_triangle.opposite_point(&triangle, p);
            let oi = opposite_triangle.point_index(op).unwrap();

            if opposite_triangle.is_constrained(oi) {
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
                result[point_idx] = opposite_triangle_id;
            }
        }

        result
    }

    /// legalize the triangle
    fn legalize(triangle_id: TriangleId, context: &mut Context, observer: &mut impl Observer) {
        observer.will_legalize(triangle_id, context);

        // keeps record of all touched triangles, after legalize finished
        // need to remap all to the advancing front
        let mut legalized_triangles = std::mem::take(&mut context.legalize_remap_tids);

        // record the task and who triggered it
        let mut task_queue = std::mem::take(&mut context.legalize_task_queue);
        task_queue.push(triangle_id);
        legalized_triangles.push(triangle_id);

        while let Some(triangle_id) = task_queue.pop() {
            for point_idx in 0..3 {
                let triangle = triangle_id.get(&context.triangles);
                // skip legalize for constrained_edge
                if triangle.is_constrained(point_idx) || triangle.is_delaunay(point_idx) {
                    continue;
                }

                let opposite_triangle_id = triangle.neighbors[point_idx];
                if opposite_triangle_id.invalid() {
                    continue;
                };
                let opposite_triangle = opposite_triangle_id.get(&context.triangles);

                let p = triangle.points[point_idx];
                let op = opposite_triangle.opposite_point(&triangle, p);

                let illegal = unsafe {
                    in_circle(
                        context.points.get_point_uncheck(p),
                        context.points.get_point_uncheck(triangle.point_ccw(p)),
                        context.points.get_point_uncheck(triangle.point_cw(p)),
                        context.points.get_point_uncheck(op),
                    )
                };
                if illegal {
                    // rotate shared edge one vertex cw to legalize it
                    let need_remap = Self::rotate_triangle_pair(
                        triangle_id,
                        p,
                        opposite_triangle_id,
                        op,
                        context.triangles,
                    );

                    // set the delaunay flag for the edge we just fixed
                    {
                        let (t, ot) = unsafe {
                            context
                                .triangles
                                .get_mut_two(triangle_id, opposite_triangle_id)
                        };

                        let (t_idx, ot_idx) = t.common_edge_index(ot).unwrap();
                        t.set_delaunay(t_idx, true);
                        ot.set_delaunay(ot_idx, true);
                    }

                    task_queue.push(triangle_id);
                    task_queue.push(opposite_triangle_id);

                    if need_remap {
                        legalized_triangles.push(triangle_id);
                        legalized_triangles.push(opposite_triangle_id);
                    }
                    break;
                } else {
                    // though we can set delaunay edge to prevent future recalulate
                    // it turns out slower, it means the recalculation is not many
                }
            }

            observer.legalize_step(triangle_id, context);
        }

        for triangle_id in legalized_triangles.drain(..) {
            Self::map_triangle_to_nodes(triangle_id, context);
        }

        {
            // give back the task queue
            context.legalize_task_queue = task_queue;
            context.legalize_remap_tids = legalized_triangles;
        }

        observer.legalized(triangle_id, context);
    }

    /// Rotate the triangle pair, returns two flag indicate (t, ot) whether candidate for af remap
    fn rotate_triangle_pair(
        t_id: TriangleId,
        p: PointId,
        ot_id: TriangleId,
        op: PointId,
        triangles: &mut Triangles,
    ) -> bool {
        let (t, ot) = unsafe { triangles.get_mut_two(t_id, ot_id) };

        let n1 = t.neighbor_ccw(p);
        let n2 = t.neighbor_cw(p);
        let n3 = ot.neighbor_ccw(op);
        let n4 = ot.neighbor_cw(op);

        let ea1 = t.edge_attr_ccw(p);
        let ea2 = t.edge_attr_cw(p);
        let ea3 = ot.edge_attr_ccw(op);
        let ea4 = ot.edge_attr_cw(op);

        // rotate shared edge one vertex cw to legalize it
        t.rotate_cw(p, op);
        ot.rotate_cw(op, p);

        t.set_edge_attr_cw(p, ea2);
        t.set_edge_attr_ccw(op, ea3);
        ot.set_edge_attr_ccw(p, ea1);
        ot.set_edge_attr_cw(op, ea4);

        t.clear_neighbors();
        ot.clear_neighbors();

        Triangles::mark_neighbor_for_two_mut(t_id, ot_id, t, ot);

        let (t, ot, t_n1, t_n2, t_n3, t_n4) =
            unsafe { triangles.get_mut_six(t_id, ot_id, n1, n2, n3, n4) };

        if let Some(t_n2) = t_n2 {
            Triangles::mark_neighbor_for_two_mut(t_id, n2, t, t_n2);
        }
        if let Some(t_n3) = t_n3 {
            Triangles::mark_neighbor_for_two_mut(t_id, n3, t, t_n3);
        }
        if let Some(t_n1) = t_n1 {
            Triangles::mark_neighbor_for_two_mut(ot_id, n1, ot, t_n1);
        }
        if let Some(t_n4) = t_n4 {
            Triangles::mark_neighbor_for_two_mut(ot_id, n4, ot, t_n4);
        }

        n1.invalid() || n2.invalid() || n3.invalid() || n4.invalid()
    }

    /// update advancing front node's triangle
    fn map_triangle_to_nodes(triangle_id: TriangleId, context: &mut Context) {
        let triangle = triangle_id.get(&context.triangles);
        for i in 0..3 {
            if triangle.neighbors[i].invalid() {
                let point = unsafe {
                    context
                        .points
                        .get_point_uncheck(triangle.point_cw(triangle.points[i]))
                };
                if let Some(node) = context.advancing_front.get_node_mut(point) {
                    node.triangle = Some(triangle_id);
                }
            }
        }
    }

    /// fill the node with one triangle.
    /// Note: The moment it filled, advancing_front is modified.
    /// if the node is covered by another triangle, then it is deleted from advancing_front.
    /// all following advancing front lookup is affected.
    fn fill_one(
        node_point: Point,
        context: &mut Context,
        observer: &mut impl Observer,
    ) -> Option<()> {
        let node = context.advancing_front.get_node(node_point).unwrap();
        let prev_node = context.advancing_front.prev_node(node_point)?;
        let next_node = context.advancing_front.next_node(node_point)?;

        let new_triangle = context.triangles.insert(InnerTriangle::new(
            prev_node.1.point_id,
            node.point_id,
            next_node.1.point_id,
        ));

        context
            .triangles
            .mark_neighbor(new_triangle, prev_node.1.triangle.unwrap());
        context
            .triangles
            .mark_neighbor(new_triangle, node.triangle.unwrap());

        // update prev_node's triangle to newly created
        context
            .advancing_front
            .insert(prev_node.1.point_id, prev_node.0, new_triangle);
        // delete the node, after fill, it is covered by new triangle
        context.advancing_front.delete(node_point);

        Self::legalize(new_triangle, context, observer);
        Some(())
    }

    fn fill_advancing_front(
        node_point: Point,
        context: &mut Context,
        observer: &mut impl Observer,
    ) {
        {
            // fill right holes
            let mut node_point = node_point;
            while let Some((next_node_point, _)) = context.advancing_front.next_node(node_point) {
                if context.advancing_front.next_node(next_node_point).is_some() {
                    // if HoleAngle exceeds 90 degrees then break
                    if Self::large_hole_dont_fill(next_node_point, &context.advancing_front) {
                        break;
                    }

                    Self::fill_one(next_node_point, context, observer);
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

                    Self::fill_one(prev_node_point, context, observer);
                    node_point = prev_node_point;
                } else {
                    break;
                }
            }
        }

        // file right basins
        if Self::basin_angle_satisfy(node_point, context) {
            Self::fill_basin(node_point, context, observer);
        }
    }

    fn large_hole_dont_fill(node_point: Point, advancing_front: &AdvancingFront) -> bool {
        let (next_point, _) = advancing_front.next_node(node_point).unwrap();
        let (prev_point, _) = advancing_front.prev_node(node_point).unwrap();

        let angle = crate::utils::Angle::new(node_point, next_point, prev_point);
        if angle.exceeds_90_degree() {
            return false;
        }
        if angle.is_negative() {
            return true;
        }

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
    fn edge_event(
        edge: Edge,
        node_point: Point,
        context: &mut Context,
        observer: &mut impl Observer,
    ) {
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

            let triangle = node.triangle.unwrap();
            if Self::try_mark_edge_for_triangle(edge.p, edge.q, triangle, context) {
                // the edge is already an edge of the triangle, return
                return;
            }

            // for now we will do all needed filling
            Self::fill_edge_event(&constrain_edge, node_point, context, observer);
        }

        // node's triangle may changed, get the latest
        let triangle = context
            .advancing_front
            .get_node(node_point)
            .unwrap()
            .triangle
            .unwrap();

        // this triangle crosses constraint so let's flippin start!
        let mut triangle_ids = std::mem::take(&mut context.triangle_id_queue);
        Self::edge_event_process(
            edge.p,
            edge.q,
            &constrain_edge,
            triangle,
            edge.q,
            &mut triangle_ids,
            context,
        );

        for triangle_id in triangle_ids.drain(..) {
            Self::legalize(triangle_id, context, observer);
        }
        context.triangle_id_queue = triangle_ids;
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
                triangle.set_constrained(index, true);

                // The triangle may or may not has a valid neighbor
                let neighbor_t_id = triangle.neighbors[index];
                if let Some(t) = triangles.get_mut(neighbor_t_id) {
                    let index = t.edge_index(p, q).unwrap();
                    t.set_constrained(index, true);
                }

                true
            }
        }
    }

    fn fill_edge_event(
        edge: &ConstrainedEdge,
        node_point: Point,
        context: &mut Context,
        observer: &mut impl Observer,
    ) {
        if edge.right {
            Self::fill_right_above_edge_event(edge, node_point, context, observer);
        } else {
            Self::fill_left_above_edge_event(edge, node_point, context, observer);
        }
    }

    fn fill_right_above_edge_event(
        edge: &ConstrainedEdge,
        mut node_point: Point,
        context: &mut Context,
        observer: &mut impl Observer,
    ) {
        while let Some((next_node_point, _)) = context.advancing_front.next_node(node_point) {
            if next_node_point.x >= edge.p.x {
                break;
            }

            // check if next node is below the edge
            if orient_2d(edge.q, next_node_point, edge.p).is_ccw() {
                Self::fill_right_below_edge_event(edge, node_point, context, observer);
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
        observer: &mut impl Observer,
    ) {
        if node_point.x >= edge.p.x {
            return;
        }

        let (next_node_point, _) = context.advancing_front.next_node(node_point).unwrap();
        let (next_next_node_point, _) = context.advancing_front.next_node(next_node_point).unwrap();

        if orient_2d(node_point, next_node_point, next_next_node_point).is_ccw() {
            // concave
            Self::fill_right_concave_edge_event(edge, node_point, context, observer);
        } else {
            // convex
            Self::fill_right_convex_edge_event(edge, node_point, context, observer);

            // retry this one
            Self::fill_right_below_edge_event(edge, node_point, context, observer);
        }
    }

    /// recursively fill concave nodes
    fn fill_right_concave_edge_event(
        edge: &ConstrainedEdge,
        node_point: Point,
        context: &mut Context,
        observer: &mut impl Observer,
    ) {
        let (node_next_point, next_node) = context.advancing_front.next_node(node_point).unwrap();
        let next_node_point_id = next_node.point_id;
        Self::fill_one(node_next_point, context, observer);

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
                    Self::fill_right_concave_edge_event(edge, node_point, context, observer);
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
        observer: &mut impl Observer,
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
            Self::fill_right_concave_edge_event(edge, node_point, context, observer);
        } else {
            // convex
            // next above or below edge?
            if orient_2d(edge.q, next_next_node_point, edge.p).is_ccw() {
                // Below
                Self::fill_right_convex_edge_event(edge, next_node_point, context, observer);
            } else {
                // Above
            }
        }
    }

    fn fill_left_above_edge_event(
        edge: &ConstrainedEdge,
        mut node_point: Point,
        context: &mut Context,
        observer: &mut impl Observer,
    ) {
        while let Some((prev_node_point, _)) = context.advancing_front.prev_node(node_point) {
            // check if next node is below the edge
            if prev_node_point.x <= edge.p.x {
                break;
            }

            if orient_2d(edge.q, prev_node_point, edge.p).is_cw() {
                Self::fill_left_below_edge_event(edge, node_point, context, observer);
            } else {
                node_point = prev_node_point;
            }
        }
    }

    fn fill_left_below_edge_event(
        edge: &ConstrainedEdge,
        node_point: Point,
        context: &mut Context,
        observer: &mut impl Observer,
    ) {
        if node_point.x > edge.p.x {
            let (prev_node_point, _) = context.advancing_front.prev_node(node_point).unwrap();
            let (prev_prev_node_point, _) =
                context.advancing_front.prev_node(prev_node_point).unwrap();
            if orient_2d(node_point, prev_node_point, prev_prev_node_point).is_cw() {
                Self::fill_left_concave_edge_event(edge, node_point, context, observer);
            } else {
                // convex
                Self::fill_left_convex_edge_event(edge, node_point, context, observer);

                // retry this one
                Self::fill_left_below_edge_event(edge, node_point, context, observer);
            }
        }
    }

    fn fill_left_convex_edge_event(
        edge: &ConstrainedEdge,
        node_point: Point,
        context: &mut Context,
        observer: &mut impl Observer,
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
            Self::fill_left_concave_edge_event(edge, prev_node_point, context, observer);
        } else {
            // convex
            // next above or below edge?
            if orient_2d(edge.q, prev_prev_node_point, edge.p).is_cw() {
                // below
                Self::fill_left_convex_edge_event(edge, prev_node_point, context, observer);
            } else {
                // above
            }
        }
    }

    fn fill_left_concave_edge_event(
        edge: &ConstrainedEdge,
        node_point: Point,
        context: &mut Context,
        observer: &mut impl Observer,
    ) {
        let (prev_node_point, _) = context.advancing_front.prev_node(node_point).unwrap();
        Self::fill_one(prev_node_point, context, observer);

        let (prev_node_point, prev_node) = context.advancing_front.prev_node(node_point).unwrap();

        if prev_node.point_id != edge.p_id() {
            // next above or below edge?
            if orient_2d(edge.q, prev_node_point, edge.p).is_cw() {
                // below
                let (prev_prev_node_point, _) =
                    context.advancing_front.prev_node(prev_node_point).unwrap();
                if orient_2d(node_point, prev_node_point, prev_prev_node_point).is_cw() {
                    // next is concave
                    Self::fill_left_concave_edge_event(edge, node_point, context, observer);
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
                triangle.set_constrained(edge_index, true);

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
                triangle.set_constrained(edge_index, true);

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
        legalize_queue: &mut Vec<TriangleId>,
        context: &mut Context,
    ) {
        assert!(!triangle_id.invalid());

        let t = context.triangles.get_unchecked(triangle_id);

        let ot_id = t.neighbor_across(p);
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
            if Self::rotate_triangle_pair(triangle_id, p, ot_id, op, &mut context.triangles) {
                Self::map_triangle_to_nodes(triangle_id, context);
                Self::map_triangle_to_nodes(ot_id, context);
            }
            // legalize later
            legalize_queue.extend([triangle_id, ot_id]);

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
                }
            } else {
                let o = orient_2d(
                    eq.get(&context.points),
                    op.get(&context.points),
                    ep.get(&context.points),
                );

                let t = Self::next_flip_triangle(o, triangle_id, ot_id, legalize_queue);
                Self::flip_edge_event(ep, eq, edge, t, p, legalize_queue, context);
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
                legalize_queue,
                context,
            );
            Self::edge_event_process(ep, eq, edge, triangle_id, p, legalize_queue, context);
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
    fn basin_angle_satisfy(node_point: Point, context: &Context) -> bool {
        const TAN_3_4_PI: f64 = -1.;
        let Some((next_point, _)) = context.advancing_front.next_node(node_point) else { return false };
        let Some((next_next_point, _)) = context.advancing_front.next_node(next_point) else { return false };

        let ax = node_point.x - next_next_point.x;
        let ay = node_point.y - next_next_point.y;
        // the basin angle is (1/2pi, pi), so as long as tan value is less than 3/4 pi's, then its angle is less than 3/4 pi

        // ay / ax < tan(3/4 * PI)
        if ax > 0. {
            ay < TAN_3_4_PI * ax
        } else {
            ay > TAN_3_4_PI * ax
        }
    }

    /// basin is like a bowl, we first identify it's left, bottom, right node.
    /// then fill it
    fn fill_basin(
        node_point: Point,
        context: &mut Context,
        observer: &mut impl Observer,
    ) -> Option<()> {
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
                width,
                left_higher,
            },
            context,
            observer,
        );

        Some(())
    }

    fn fill_basin_req(
        node: Point,
        basin: &Basin,
        context: &mut Context,
        observer: &mut impl Observer,
    ) -> Option<()> {
        if basin.completed(node) {
            return None;
        }

        Self::fill_one(node, context, observer);

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

        Self::fill_basin_req(new_node, basin, context, observer)
    }
}

impl Sweeper {
    pub fn verify_triangles(context: &Context) -> bool {
        Self::illegal_triangles(context).is_empty()
    }

    /// verify all triangles stored in context are legal
    #[allow(unused)]
    pub fn illegal_triangles(context: &Context) -> Vec<(TriangleId, TriangleId)> {
        let triangle_ids = context
            .triangles
            .iter()
            .map(|(t_id, _)| t_id)
            .collect::<Vec<_>>();

        let mut result = Vec::<(TriangleId, TriangleId)>::new();

        for t_id in triangle_ids {
            for illegal_neighbor in &Self::is_legalize(t_id, context) {
                if !illegal_neighbor.invalid() {
                    result.push((t_id, *illegal_neighbor));
                }
            }
        }

        result
    }
}

fn parse_polyline(polyline: Vec<Point>, points: &mut Points) -> Vec<Edge> {
    let mut edge_list = Vec::with_capacity(polyline.len());

    let mut point_iter = polyline.iter().map(|p| (points.add_point(*p), p));
    if let Some(first_point) = point_iter.next() {
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
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use rand::Rng;

    use super::*;

    #[test]
    fn test_bird() {
        let file_path = "test_data/bird.dat";
        let points = try_load_from_file(file_path).unwrap();

        let sweeper = SweeperBuilder::new(points).build();
        sweeper.triangulate();
    }

    #[test]
    fn test_rand() {
        let test_path = "test_data/latest_test_data";
        let points = match try_load_from_file(test_path) {
            None => {
                let mut points = Vec::<Point>::new();
                for _ in 0..100 {
                    let x: f64 = rand::thread_rng().gen_range(0.0..800.);
                    let y: f64 = rand::thread_rng().gen_range(0.0..800.);
                    points.push(Point::new(x, y));
                }
                save_to_file(&points, test_path);
                points
            }
            Some(points) => points,
        };

        let sweeper = SweeperBuilder::new(vec![
            Point::new(-10., -10.),
            Point::new(810., -10.),
            Point::new(810., 810.),
            Point::new(-10., 810.),
        ])
        .add_steiner_points(points)
        .add_hole(vec![
            Point::new(400., 400.),
            Point::new(600., 400.),
            Point::new(600., 600.),
            Point::new(400., 600.),
        ])
        .build();
        sweeper.triangulate();

        delete_file(test_path);
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