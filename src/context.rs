use crate::{
    shape::{Point, Triangle},
    AdvancingFront, Edges, Points, TriangleId, Triangles,
};
use rustc_hash::FxHashSet;
use rusttype::Scale;

pub struct FillContext<'a> {
    pub points: &'a Points,
    pub edges: &'a Edges,
    pub triangles: &'a mut Triangles,
    pub advancing_front: &'a mut AdvancingFront,
    pub map: &'a mut FxHashSet<TriangleId>,
    pub result: Vec<TriangleId>,
}

impl FillContext<'_> {
    pub fn debug_with_msg(&self, msg: &str) {
        println!("vv {msg}");
        self.debug();
    }

    pub fn debug(&self) {
        println!("vvvvvvvvvvvvvvvvvvv");
        print!("  points: ");
        for (id, p) in self.points.iter() {
            print!("{} ", point_debug(*p));
        }
        println!(
            "h:{} t:{}",
            point_debug(self.points.head),
            point_debug(self.points.tail),
        );

        println!("  triangles:");
        for t in self.triangles.iter() {
            println!(
                "    <{}, {}, {}> ",
                point_debug(self.points.get_point(t.points[0]).unwrap()),
                point_debug(self.points.get_point(t.points[1]).unwrap()),
                point_debug(self.points.get_point(t.points[2]).unwrap()),
            );
        }
        println!("  advancing: {:?}", self.advancing_front);

        fn point_debug(p: Point) -> String {
            format!("({}, {})", p.x, p.y)
        }
    }

    pub fn draw(&self) {
        use image::{Rgb, RgbImage};
        use imageproc::drawing::*;
        use imageproc::rect::Rect;
        use rusttype::Font;

        let red = Rgb([255u8, 0u8, 0u8]);
        let green = Rgb([0u8, 255u8, 0u8]);
        let blue = Rgb([0u8, 0u8, 255u8]);
        let white = Rgb([255u8, 255u8, 255u8]);
        let black = Rgb([0u8, 0, 0]);
        let gray = Rgb([180u8, 180, 180]);
        let yellow = Rgb([255u8, 255, 0]);

        let mut image = RgbImage::new(800, 800);
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

            fn map_size(&self, w: f64, h: f64) -> (f64, f64) {
                (w / self.from.w * self.to.w, h / self.from.h * self.to.h)
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
        for p in self
            .points
            .iter()
            .map(|(_, p)| p)
            .chain(&[self.points.head, self.points.tail])
        {
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
                w: 800.,
                h: 800.,
            },
        };

        let point_size = 10.;

        for (id, point) in self.points.iter() {
            let (x, y) = map.map_point(point.x - point_size / 2., point.y + point_size / 2.);

            draw_text_mut(
                &mut image,
                red,
                x as i32,
                y as i32,
                Scale::uniform(10.),
                &font,
                &format!("{:.2}, {:.2}", point.x, point.y),
            );

            for p_id in self.edges.p_for_q(id) {
                let p_point = self.points.get_point(*p_id).unwrap();
                let p = map.map_point_f32(p_point.x, p_point.y);
                let q = map.map_point_f32(point.x, point.y);
                draw_line_segment_mut(&mut image, p, q, yellow);
            }
        }

        for t in self.triangles.iter() {
            let p0 = self.points.get_point(t.points[0]).unwrap();
            let p1 = self.points.get_point(t.points[1]).unwrap();
            let p2 = self.points.get_point(t.points[2]).unwrap();

            let p0 = map.map_point_f32(p0.x, p0.y);
            let p1 = map.map_point_f32(p1.x, p1.y);
            let p2 = map.map_point_f32(p2.x, p2.y);

            draw_line_segment_mut(&mut image, p0, p1, blue);
            draw_line_segment_mut(&mut image, p1, p2, blue);
            draw_line_segment_mut(&mut image, p2, p0, blue);
        }

        for (p, _n) in self.advancing_front.iter() {
            let (x, y) = map.map_point_i32(p.x, p.y);
            let (w, h) = map.map_size(point_size, point_size);
            let rect = Rect::at(x as i32, y as i32).of_size(w as u32, h as u32);
            draw_hollow_rect_mut(&mut image, rect, red);
        }

        for t in &self.result {
            let t = self.triangles.get(*t).unwrap();

            let p0 = self.points.get_point(t.points[0]).unwrap();
            let p1 = self.points.get_point(t.points[1]).unwrap();
            let p2 = self.points.get_point(t.points[2]).unwrap();

            let p0 = map.map_point_f32(p0.x, p0.y);
            let p1 = map.map_point_f32(p1.x, p1.y);
            let p2 = map.map_point_f32(p2.x, p2.y);

            draw_line_segment_mut(&mut image, p0, p1, red);
            draw_line_segment_mut(&mut image, p1, p2, red);
            draw_line_segment_mut(&mut image, p2, p0, red);
        }

        static DRAW_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let draw_id = DRAW_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = format!("test_files/context_dump_{}.png", draw_id);
        image.save(&path).unwrap();
    }
}