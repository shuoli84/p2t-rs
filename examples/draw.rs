use clap::Parser;
use poly2tri_rs::{
    loader::{Loader, PlainFileLoader},
    Context, Edge, Observer, Point, Sweeper, SweeperBuilder, TriangleId,
};
use rand::Rng;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    path: Option<std::path::PathBuf>,

    #[arg(long)]
    detail: bool,

    #[arg(long, default_value = "true")]
    result: bool,

    #[arg(long, default_value = "1")]
    count: usize,

    #[arg(long, default_value = "false")]
    test: bool,

    #[arg(long, default_value = "false")]
    debug: bool,
}

fn try_load_from_file(path: &std::path::PathBuf) -> Option<Vec<Point>> {
    let mut f = std::fs::File::options().read(true).open(path).ok()?;
    let mut value = "".to_string();
    std::io::Read::read_to_string(&mut f, &mut value).unwrap();
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

fn main() {
    let args = Args::parse();

    let sweeper = if args.test {
        let points = if let Some(path) = args.path.as_ref() {
            try_load_from_file(path).unwrap()
        } else {
            let mut points = Vec::<Point>::new();
            for _ in 0..100 {
                let x: f64 = rand::thread_rng().gen_range(0.0..800.);
                let y: f64 = rand::thread_rng().gen_range(0.0..800.);
                points.push(Point::new(x, y));
            }
            points
        };

        SweeperBuilder::new(vec![
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
        .build()
    } else {
        let mut file_loader = PlainFileLoader::default();
        file_loader
            .load(args.path.as_ref().unwrap().as_os_str().to_str().unwrap())
            .unwrap()
    };

    for _i in 0..args.count {
        let _result = sweeper
            .clone()
            .triangulate_with_observer(&mut DrawObserver::new(&args));
    }
}

struct DrawObserver {
    messages: Vec<String>,
    // whether show debug info, like point_id, locations, triangle, messages
    debug: bool,
    detail: bool,
    result: bool,
    inspect_id: Option<usize>,
    legalizing: Option<TriangleId>,
    prev_detail: bool,
    draw_options: DrawOptions,
}

struct DrawOptions {
    // whether draw all triangles
    draw_triangles: bool,
    // whether draw result
    draw_result: bool,
}

impl Default for DrawOptions {
    fn default() -> Self {
        Self {
            draw_result: true,
            draw_triangles: true,
        }
    }
}

impl DrawObserver {
    fn new(args: &Args) -> Self {
        Self {
            debug: args.debug,
            detail: args.detail,
            result: args.result,
            messages: Default::default(),
            inspect_id: Default::default(),
            legalizing: Default::default(),
            prev_detail: Default::default(),
            draw_options: DrawOptions::default(),
        }
    }
}

impl Observer for DrawObserver {
    fn point_event(&mut self, point_id: poly2tri_rs::PointId, context: &Context) {
        if self.detail {
            let point = context.points.get_point(point_id).unwrap();
            self.messages
                .push(format!("point event: {point_id:?} {point:?}"));
            self.draw(context);
        }
    }

    fn edge_event(&mut self, edge: Edge, context: &Context) {
        if self.detail {
            self.messages.push(format!(
                "edge_event: p:{} q:{}",
                edge.p.as_usize(),
                edge.q.as_usize(),
            ));

            self.draw(context);
        }
    }

    fn will_legalize(&mut self, triangle_id: TriangleId, context: &Context) {
        self.legalizing = Some(triangle_id);
        if self
            .inspect_id
            .map(|i| i == triangle_id.as_usize())
            .unwrap_or_default()
        {
            self.prev_detail = self.detail;
            self.detail = true;
        }

        if self.detail && self.inspect_id.is_some() {
            self.messages.push(format!(
                "will legalize triangle: {}",
                triangle_id.as_usize()
            ));
            self.draw(context);
        }
    }

    fn legalize_step(&mut self, triangle_id: TriangleId, context: &Context) {
        if self.detail && self.inspect_id.is_some() {
            self.messages.push(format!(
                "leaglize step for t:{} this t:{}",
                self.legalizing.unwrap().as_usize(),
                triangle_id.as_usize()
            ));
            self.draw(context);
        }
    }

    fn legalized(&mut self, triangle_id: TriangleId, context: &Context) {
        if self.detail {
            self.messages
                .push(format!("leaglized triangle: {}", triangle_id.as_usize()));
            self.draw(context);
        }

        if let Some(id) = self.inspect_id {
            if id == triangle_id.as_usize() {
                self.detail = self.prev_detail;
            }
        }
        self.legalizing = None;
    }

    fn sweep_done(&mut self, context: &Context) {
        if self.detail {
            self.messages.push("sweep done".into());
            self.draw(context);
        }
    }

    fn finalized(&mut self, context: &Context) {
        if self.result {
            self.messages.push("finalized".into());
            self.draw(context);
        }
    }
}

impl DrawObserver {
    fn draw(&mut self, context: &Context) {
        use svg::Document;
        use svg::Node;

        #[derive(Debug, Clone, Copy)]
        struct MapRect {
            x: f64,
            y: f64,
            w: f64,
            h: f64,
        }

        // map rect with y flipped, svg's coordinate with origin at left-top
        #[derive(Debug)]
        struct Map {
            from: MapRect,
            to: MapRect,
        }

        impl Map {
            fn map_point(&self, x: f64, y: f64) -> (f64, f64) {
                let x = (x - self.from.x) / self.from.w * self.to.w + self.to.x;
                let y = self.to.h - (y - self.from.y) / self.from.h * self.to.h + self.to.y;
                (x, y)
            }
        }

        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        for p in context.points.iter().map(|(_, p)| p) {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }

        let from = MapRect {
            x: min_x - 30.,
            y: min_y - 30.,
            w: max_x - min_x + 60.,
            h: max_y - min_y + 60.,
        };
        let map = Map { from, to: from };

        let mut doc = Document::new()
            .set("viewBox", (from.x, from.y, from.w, from.h))
            .set("style", "background-color: #F5F5F5");

        for (id, point) in context.points.iter() {
            let (x, y) = map.map_point(point.x, point.y);

            if self.debug {
                doc.append(text(
                    format!("({}) ({:.2}, {:.2})", id.as_usize(), point.x, point.y),
                    (x, y),
                ));
            }

            doc.append(circle((x, y), 3., "red", "clear"));

            for p_id in context.edges.p_for_q(id) {
                let p_point = context.points.get_point(*p_id).unwrap();
                let p = map.map_point(p_point.x, p_point.y);
                let q = map.map_point(point.x, point.y);

                doc.append(line(p, q, "black"));
            }
        }

        if self.draw_options.draw_triangles {
            for (id, t) in context.triangles.iter() {
                let p0 = context.points.get_point(t.points[0]).unwrap();
                let p1 = context.points.get_point(t.points[1]).unwrap();
                let p2 = context.points.get_point(t.points[2]).unwrap();

                let p0 = map.map_point(p0.x, p0.y);
                let p1 = map.map_point(p1.x, p1.y);
                let p2 = map.map_point(p2.x, p2.y);

                doc.append(triangle(p0, p1, p2, "blue", "clear"));

                let center = ((p0.0 + p1.0 + p2.0) / 3., (p0.1 + p1.1 + p2.1) / 3.);

                let point_percent = 0.5;
                let center_percent = 1. - point_percent;

                if self.debug {
                    let p0_drifted = (
                        center.0 * center_percent + p0.0 * point_percent,
                        center.1 * center_percent + p0.1 * point_percent,
                    );
                    let p1_drifted = (
                        center.0 * center_percent + p1.0 * point_percent,
                        center.1 * center_percent + p1.1 * point_percent,
                    );
                    let p2_drifted = (
                        center.0 * center_percent + p2.0 * point_percent,
                        center.1 * center_percent + p2.1 * point_percent,
                    );

                    let color_for_idx = |idx: usize| {
                        let color = if t.is_constrained(idx) {
                            "yellow"
                        } else {
                            "gray"
                        };
                        let color = if t.neighbors[idx].invalid() {
                            "red"
                        } else {
                            color
                        };
                        let color = if t.is_delaunay(idx) { "black" } else { color };
                        color
                    };

                    doc.append(line(p0_drifted, p1_drifted, color_for_idx(2)));
                    doc.append(line(p1_drifted, p2_drifted, color_for_idx(0)));
                    doc.append(line(p2_drifted, p0_drifted, color_for_idx(1)));

                    doc.append(text(
                        format!("{}", id.as_usize()),
                        ((p0.0 + p1.0 + p2.0) / 3., (p0.1 + p1.1 + p2.1) / 3.),
                    ));
                }
            }
        }

        if self.debug {
            for (_p, n) in context.advancing_front.iter() {
                if let Some(t) = n.triangle {
                    let t = context.triangles.get(t).unwrap();

                    let p0 = context.points.get_point(t.points[0]).unwrap();
                    let p1 = context.points.get_point(t.points[1]).unwrap();
                    let p2 = context.points.get_point(t.points[2]).unwrap();

                    let p0 = map.map_point(p0.x, p0.y);
                    let p1 = map.map_point(p1.x, p1.y);
                    let p2 = map.map_point(p2.x, p2.y);

                    doc.append(line(p0, p1, "red"));
                    doc.append(line(p1, p2, "red"));
                    doc.append(line(p2, p0, "red"));
                }
            }
        }

        if self.draw_options.draw_result {
            for t in &context.result {
                let t = context.triangles.get(*t).unwrap();

                let p0 = context.points.get_point(t.points[0]).unwrap();
                let p1 = context.points.get_point(t.points[1]).unwrap();
                let p2 = context.points.get_point(t.points[2]).unwrap();

                let p0 = map.map_point(p0.x, p0.y);
                let p1 = map.map_point(p1.x, p1.y);
                let p2 = map.map_point(p2.x, p2.y);

                doc.append(triangle(p0, p1, p2, "white", "blue"));
            }
        }

        if self.debug {
            let mut y = 40;
            let mut messages = svg::node::element::Group::new();
            for m in std::mem::take(&mut self.messages).into_iter() {
                messages.append(text(m, (0., y as f64)));
                y += 40;
            }
            doc.append(messages.set("x", 50));
        } else {
            self.messages.clear();
        }

        let mut draw_illegal_triangle = |tid: TriangleId, fill_color: &str, border_color: &str| {
            let t = tid.get(&context.triangles);
            let p0 = context.points.get_point(t.points[0]).unwrap();
            let p1 = context.points.get_point(t.points[1]).unwrap();
            let p2 = context.points.get_point(t.points[2]).unwrap();

            {
                let p0 = map.map_point(p0.x, p0.y);
                let p1 = map.map_point(p1.x, p1.y);
                let p2 = map.map_point(p2.x, p2.y);

                doc.append(triangle(p0, p1, p2, fill_color, border_color));
            }
        };

        let illegal_pairs = Sweeper::illegal_triangles(context);
        for (from_tid, to_tid) in illegal_pairs {
            draw_illegal_triangle(from_tid, "red", "black");
            draw_illegal_triangle(to_tid, "yellow", "black");
        }

        static DRAW_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let draw_id = DRAW_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = format!("test_files/context_dump_{}.svg", draw_id);
        svg::save(path, &doc).unwrap();
    }
}

fn line(p: (f64, f64), q: (f64, f64), color: &str) -> svg::node::element::Line {
    svg::node::element::Line::new()
        .set("class", "edge")
        .set("stroke", to_color(color))
        .set("x1", p.0)
        .set("y1", p.1)
        .set("x2", q.0)
        .set("y2", q.1)
}

fn text(content: impl Into<String>, p: (f64, f64)) -> svg::node::element::Text {
    svg::node::element::Text::new()
        .add(svg::node::Text::new(content))
        .set("x", p.0)
        .set("y", p.1)
}

fn triangle(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    border_color: &str,
    fill_color: &str,
) -> svg::node::element::Path {
    let data = svg::node::element::path::Data::new()
        .move_to(p0)
        .line_to(p1)
        .line_to(p2)
        .close();
    svg::node::element::Path::new()
        .set("d", data)
        .set("stroke", to_color(border_color))
        .set("fill", to_color(fill_color))
}

fn circle(
    c: (f64, f64),
    r: f64,
    stroke_color: &str,
    fill_color: &str,
) -> svg::node::element::Circle {
    svg::node::element::Circle::new()
        .set("cx", c.0)
        .set("cy", c.1)
        .set("r", r)
        .set("stroke-color", to_color(stroke_color))
        .set("fill-color", to_color(fill_color))
}

fn to_color(name: &str) -> String {
    match name {
        "blue" => "#29B6F6",
        "yellow" => "#FFA726",
        "red" => "#EF5350",
        "black" => "#3E2723",
        "gray" => "#616161",
        "clear" => "#00000000",
        _ => name,
    }
    .into()
}
