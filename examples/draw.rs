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

    #[arg(short, long)]
    detail: bool,

    #[arg(short, long)]
    result: bool,

    #[arg(short, long, default_value = "1")]
    count: usize,

    #[arg(short, long, default_value = "false")]
    test: bool,
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
            .load(args.path.unwrap().as_os_str().to_str().unwrap())
            .unwrap()
    };

    for _i in 0..args.count {
        let _result = sweeper.clone().triangulate_with_observer(DrawObserver {
            messages: vec![],
            detail: args.detail,
            result: args.result,
            legalizing: None,
            prev_detail: args.detail,
            inspect_id: None,
        });
    }
}

#[derive(Default)]
struct DrawObserver {
    messages: Vec<String>,
    detail: bool,
    result: bool,
    inspect_id: Option<usize>,
    legalizing: Option<TriangleId>,
    prev_detail: bool,
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
        use image::{Rgb, RgbImage};
        use imageproc::drawing::*;
        use imageproc::point::Point;
        use rusttype::Font;
        use rusttype::Scale;

        let red = Rgb([255u8, 0u8, 0u8]);
        let blue = Rgb([0u8, 0u8, 255u8]);
        let black = Rgb([0u8, 0, 0]);
        let gray = Rgb([180u8, 180, 180]);
        let yellow = Rgb([255u8, 255, 0]);

        // 1600 picture, 800 messages
        let mut image = RgbImage::new(2400, 1600);
        image.fill(255);

        let font = Vec::from(include_bytes!("../test_files/DejaVuSans.ttf") as &[u8]);
        let font = Font::try_from_vec(font).unwrap();

        #[derive(Debug)]
        struct MapRect {
            x: f64,
            y: f64,
            w: f64,
            h: f64,
        }

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

            fn map_point_i32(&self, x: f64, y: f64) -> (i32, i32) {
                let (x, y) = self.map_point(x, y);
                (x as i32, y as i32)
            }

            fn map_point_f32(&self, x: f64, y: f64) -> (f32, f32) {
                let (x, y) = self.map_point(x, y);
                (x as f32, y as f32)
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

        let map = Map {
            from: MapRect {
                x: min_x - 30.,
                y: min_y - 30.,
                w: max_x - min_x + 60.,
                h: max_y - min_y + 60.,
            },
            to: MapRect {
                x: 0.,
                y: 0.,
                w: 1600.,
                h: 1600.,
            },
        };

        let point_size = 10.;

        for (id, point) in context.points.iter() {
            let (x, y) = map.map_point(point.x - point_size / 2., point.y + point_size / 2.);

            draw_text_mut(
                &mut image,
                red,
                x as i32,
                y as i32,
                Scale::uniform(10.),
                &font,
                &format!("({}) ({:.2}, {:.2})", id.as_usize(), point.x, point.y),
            );

            for p_id in context.edges.p_for_q(id) {
                let p_point = context.points.get_point(*p_id).unwrap();
                let p = map.map_point_f32(p_point.x, p_point.y);
                let q = map.map_point_f32(point.x, point.y);
                draw_line_segment_mut(&mut image, p, q, yellow);
            }
        }

        for (id, t) in context.triangles.iter() {
            let p0 = context.points.get_point(t.points[0]).unwrap();
            let p1 = context.points.get_point(t.points[1]).unwrap();
            let p2 = context.points.get_point(t.points[2]).unwrap();

            let p0 = map.map_point_f32(p0.x, p0.y);
            let p1 = map.map_point_f32(p1.x, p1.y);
            let p2 = map.map_point_f32(p2.x, p2.y);
            let center = ((p0.0 + p1.0 + p2.0) / 3., (p0.1 + p1.1 + p2.1) / 3.);

            let point_percent = 0.5;
            let center_percent = 1. - point_percent;

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
                let color = if t.is_constrained(idx) { yellow } else { gray };
                let color = if t.neighbors[idx].invalid() {
                    red
                } else {
                    color
                };
                let color = if t.is_delaunay(idx) { black } else { color };
                color
            };

            draw_line_segment_mut(&mut image, p0_drifted, p1_drifted, color_for_idx(2));
            draw_line_segment_mut(&mut image, p1_drifted, p2_drifted, color_for_idx(0));
            draw_line_segment_mut(&mut image, p2_drifted, p0_drifted, color_for_idx(1));

            draw_line_segment_mut(&mut image, p0, p1, blue);
            draw_line_segment_mut(&mut image, p1, p2, blue);
            draw_line_segment_mut(&mut image, p2, p0, blue);

            draw_text_mut(
                &mut image,
                black,
                ((p0.0 + p1.0 + p2.0) / 3.) as i32,
                ((p0.1 + p1.1 + p2.1) / 3.) as i32,
                Scale::uniform(10.),
                &font,
                format!("{}", id.as_usize()).as_str(),
            );
        }

        for (_p, n) in context.advancing_front.iter() {
            if let Some(t) = n.triangle {
                let t = context.triangles.get(t).unwrap();

                let p0 = context.points.get_point(t.points[0]).unwrap();
                let p1 = context.points.get_point(t.points[1]).unwrap();
                let p2 = context.points.get_point(t.points[2]).unwrap();

                let p0 = map.map_point_f32(p0.x, p0.y);
                let p1 = map.map_point_f32(p1.x, p1.y);
                let p2 = map.map_point_f32(p2.x, p2.y);

                draw_line_segment_mut(&mut image, p0, p1, red);
                draw_line_segment_mut(&mut image, p1, p2, red);
                draw_line_segment_mut(&mut image, p2, p0, red);
            }
        }

        for t in &context.result {
            let t = context.triangles.get(*t).unwrap();

            let p0 = context.points.get_point(t.points[0]).unwrap();
            let p1 = context.points.get_point(t.points[1]).unwrap();
            let p2 = context.points.get_point(t.points[2]).unwrap();

            {
                let p0 = map.map_point_i32(p0.x, p0.y);
                let p1 = map.map_point_i32(p1.x, p1.y);
                let p2 = map.map_point_i32(p2.x, p2.y);

                draw_polygon_mut(
                    &mut image,
                    &[
                        Point::new(p0.0, p0.1),
                        Point::new(p1.0, p1.1),
                        Point::new(p2.0, p2.1),
                    ],
                    blue,
                );
            }
            {
                let p0 = map.map_point_f32(p0.x, p0.y);
                let p1 = map.map_point_f32(p1.x, p1.y);
                let p2 = map.map_point_f32(p2.x, p2.y);

                draw_line_segment_mut(&mut image, p0, p1, red);
                draw_line_segment_mut(&mut image, p1, p2, red);
                draw_line_segment_mut(&mut image, p2, p0, red);
            }
        }

        let mut y = 40;
        for m in std::mem::take(&mut self.messages).into_iter() {
            draw_text_mut(&mut image, black, 1600, y, Scale::uniform(20.), &font, &m);
            y += 40;
        }

        let mut draw_triangle = |tid: TriangleId, fill_color: Rgb<u8>, border_color: Rgb<u8>| {
            let t = tid.get(&context.triangles);
            let p0 = context.points.get_point(t.points[0]).unwrap();
            let p1 = context.points.get_point(t.points[1]).unwrap();
            let p2 = context.points.get_point(t.points[2]).unwrap();

            {
                let p0 = map.map_point_i32(p0.x, p0.y);
                let p1 = map.map_point_i32(p1.x, p1.y);
                let p2 = map.map_point_i32(p2.x, p2.y);

                draw_polygon_mut(
                    &mut image,
                    &[
                        Point::new(p0.0, p0.1),
                        Point::new(p1.0, p1.1),
                        Point::new(p2.0, p2.1),
                    ],
                    fill_color,
                );
            }
            {
                let p0 = map.map_point_f32(p0.x, p0.y);
                let p1 = map.map_point_f32(p1.x, p1.y);
                let p2 = map.map_point_f32(p2.x, p2.y);

                draw_line_segment_mut(&mut image, p0, p1, border_color);
                draw_line_segment_mut(&mut image, p1, p2, border_color);
                draw_line_segment_mut(&mut image, p2, p0, border_color);
            }
        };

        let illegal_pairs = Sweeper::illegal_triangles(context);
        for (from_tid, to_tid) in illegal_pairs {
            draw_triangle(from_tid, red, black);
            draw_triangle(to_tid, yellow, black);
        }

        static DRAW_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let draw_id = DRAW_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = format!("test_files/context_dump_{}.png", draw_id);
        image.save(&path).unwrap();
    }
}
