//! A resize handle for uniformly scaling a plugin GUI.

use vizia::prelude::*;
use vizia::vg;

/// A resize handle placed at the bottom right of the window that lets you resize the window.
///
/// Needs to be the last element in the GUI because of how event targetting in Vizia works right
/// now.
pub struct ResizeHandle {
    /// Will be set to `true` if we're dragging the parameter. Resetting the parameter or entering a
    /// text value should not initiate a drag.
    drag_active: bool,

    /// The scale factor when we started dragging. This is kept track of separately to avoid
    /// accumulating rounding errors.
    start_scale_factor: f64,
    /// The DPI factor when we started dragging, includes both the HiDPI scaling and the user
    /// scaling factor. This is kept track of separately to avoid accumulating rounding errors.
    start_physical_size: (f32,f32),
    /// The cursor position in physical screen pixels when the drag started.
    start_physical_coordinates: (f32, f32),
}

impl ResizeHandle {
    /// Create a resize handle at the bottom right of the window. This should be created at the top
    /// level. Dragging this handle around will cause the window to be resized.
    pub fn new(cx: &mut Context) -> Handle<Self> {
        // Styling is done in the style sheet
        ResizeHandle {
            drag_active: false,
            start_scale_factor: 1.0,
            start_physical_size: (1.0,1.0),
            start_physical_coordinates: (0.0, 0.0),
        }
        .build(cx, |_| {})
    }
}

impl View for ResizeHandle {
    fn element(&self) -> Option<&'static str> {
        Some("resize-handle")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match *window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                // The handle is a triangle, so we should also interact with it as if it was a
                // triangle
                if intersects_triangle(
                    cx.cache.get_bounds(cx.current()),
                    (cx.mouse().cursorx, cx.mouse().cursory),
                ) {
                    cx.capture();
                    cx.set_active(true);

                    self.drag_active = true;
                    self.start_scale_factor = cx.user_scale_factor();
                    let root=cx.cache.get_bounds(Entity::root());
                    self.start_physical_size=(root.w.max(1.0),root.h.max(1.0));
                    self.start_physical_coordinates=(cx.mouse().cursorx,cx.mouse().cursory);

                    meta.consume();
                } else {
                    // TODO: The click should be forwarded to the element behind the triangle
                }
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.drag_active {
                    cx.release();
                    cx.set_active(false);

                    self.drag_active = false;
                }
            }
            WindowEvent::MouseMove(x, y) => {
                cx.set_hover(intersects_triangle(
                    cx.cache.get_bounds(cx.current()),
                    (x, y),
                ));

                if self.drag_active {
                    // We need to convert our measurements into physical pixels relative to the
                    // initial drag to be able to keep a consistent ratio. This 'relative to the
                    // start' bit is important because otherwise we would be comparing the position
                    // to the same absoltue screen spotion.
                    // TODO: This may start doing fun things when the window grows so large that it
                    //       gets pushed upwards or leftwards
                    let new_scale_factor=drag_scale(self.start_scale_factor,
                        self.start_physical_size,self.start_physical_coordinates,(x,y));

                    // If this is different then the window will automatically be resized at the end
                    // of the frame
                    cx.set_user_scale_factor(new_scale_factor);
                }
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        // We'll draw the handle directly as styling elements for this is going to be a bit tricky

        // These basics are taken directly from the default implementation of this function
        let bounds = cx.bounds();
        if bounds.w == 0.0 || bounds.h == 0.0 {
            return;
        }

        let background_color = cx.background_color();
        let border_color = cx.border_color();
        let opacity = cx.opacity();
        let mut background_color: vg::Color = background_color.into();
        background_color.set_alphaf(background_color.a * opacity);
        let mut border_color: vg::Color = border_color.into();
        border_color.set_alphaf(border_color.a * opacity);
        let border_width = cx.border_width();

        let mut path = vg::Path::new();
        let x = bounds.x + border_width / 2.0;
        let y = bounds.y + border_width / 2.0;
        let w = bounds.w - border_width;
        let h = bounds.h - border_width;
        path.move_to(x, y);
        path.line_to(x, y + h);
        path.line_to(x + w, y + h);
        path.line_to(x + w, y);
        path.line_to(x, y);
        path.close();

        // Fill with background color
        let paint = vg::Paint::color(background_color);
        canvas.fill_path(&path, &paint);

        // Borders are only supported to make debugging easier
        let mut paint = vg::Paint::color(border_color);
        paint.set_line_width(border_width);
        canvas.stroke_path(&path, &paint);

        // We'll draw a simple triangle, since we're going flat everywhere anyways and that style
        // tends to not look too tacky
        let mut path = vg::Path::new();
        let x = bounds.x + border_width / 2.0;
        let y = bounds.y + border_width / 2.0;
        let w = bounds.w - border_width;
        let h = bounds.h - border_width;
        path.move_to(x, y + h);
        path.line_to(x + w, y + h);
        path.line_to(x + w, y);
        path.move_to(x, y + h);
        path.close();

        // Yeah this looks nowhere as good
        // path.move_to(x, y + h);
        // path.line_to(x + (w / 3.0), y + h);
        // path.line_to(x + w, y + h / 3.0);
        // path.line_to(x + w, y);
        // path.move_to(x, y + h);
        // path.close();

        // path.move_to(x + (w / 3.0 * 1.5), y + h);
        // path.line_to(x + (w / 3.0 * 2.5), y + h);
        // path.line_to(x + w, y + (h / 3.0 * 2.5));
        // path.line_to(x + w, y + (h / 3.0 * 1.5));
        // path.move_to(x + (w / 3.0 * 1.5), y + h);
        // path.close();

        let mut color: vg::Color = cx.font_color().into();
        color.set_alphaf(color.a * opacity);
        let paint = vg::Paint::color(color);
        canvas.fill_path(&path, &paint);
    }
}

/// Project pointer displacement onto a proportional resize, in physical units.
fn drag_scale(start:f64,(w,h):(f32,f32),(sx,sy):(f32,f32),(x,y):(f32,f32))->f64 {
    let ratio=1.0+((x-sx)*w+(y-sy)*h)/(w*w+h*h).max(1.0);
    (start*ratio as f64).clamp(0.5,2.0)
}

/// Test whether a point intersects with the triangle of this resize handle.
fn intersects_triangle(bounds: BoundingBox, (x, y): (f32, f32)) -> bool {
    // We could also compute Barycentric coordinates, but this is simple and I like not having to
    // think. Just check if (going clockwise), the point is on the right of each of all of the
    // triangle's edges. We can compute this using the determinant of the 2x2 matrix formed by two
    // column vectors, aka the perp dot product, aka the wedge product.
    // NOTE: Since this element is positioned in the bottom right corner we would technically only
    //       have to calculate this for `v1`
    let (p1x, p1y) = bounds.bottom_left();
    let (p2x, p2y) = bounds.top_right();
    // let (p3x, p3y) = bounds.bottom_right();

    let (v1x, v1y) = (p2x - p1x, p2y - p1y);
    // let (v2x, v2y) = (p3x - p2x, p3y - p2y);
    // let (v3x, v3y) = (p1x - p3x, p1y - p3y);

    ((x - p1x) * v1y) - ((y - p1y) * v1x) >= 0.0
    // && ((x - p2x) * v2y) - ((y - p2y) * v2x) >= 0.0
    // && ((x - p3x) * v3y) - ((y - p3y) * v3x) >= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_tracks_relative_motion_and_is_dpi_independent() {
        for dpi in [1.0,2.0] {
            let size=(1360.0*dpi,565.0*dpi);let pointer=(1355.0*dpi,560.0*dpi);
            let new=(pointer.0+size.0*0.25,pointer.1+size.1*0.25);
            assert!((drag_scale(1.0,size,pointer,new)-1.25).abs()<1e-6);
            assert_eq!(drag_scale(1.0,size,pointer,pointer),1.0);
            assert!(drag_scale(1.0,size,pointer,(pointer.0-100.0*dpi,pointer.1))<1.0);
        }
    }
    #[test]
    fn actual_handle_drag_updates_user_zoom_and_renderer_dpi_together() {
        use vizia::context::backend::BackendContext;
        let mut cx=Context::new(WindowSize::new(1360,565),1.0);
        let e=ResizeHandle::new(&mut cx).entity();let mut backend=BackendContext::new_with_event_manager(&mut cx);
        backend.set_scale_factor(2.0);
        backend.cache().set_bounds(Entity::root(),BoundingBox{x:0.0,y:0.0,w:2720.0,h:1130.0});
        backend.cache().set_bounds(e,BoundingBox{x:2680.0,y:1090.0,w:40.0,h:40.0});
        backend.emit_origin(WindowEvent::MouseMove(2715.0,1125.0));backend.process_events();
        let send=|backend:&mut BackendContext,event| {
            backend.send_event(Event::new(event).target(e).origin(e).propagate(Propagation::Direct));backend.process_events();
        };
        send(&mut backend,WindowEvent::MouseDown(MouseButton::Left));
        send(&mut backend,WindowEvent::MouseMove(3395.0,1407.5));
        assert_eq!(backend.user_scale_factor(),1.25);assert_eq!(backend.scale_factor(),2.5);
        send(&mut backend,WindowEvent::MouseUp(MouseButton::Left));
        send(&mut backend,WindowEvent::MouseMove(2000.0,900.0));
        assert_eq!(backend.user_scale_factor(),1.25);
    }

    #[test]
    fn triangle_intersection() {
        let bbox = BoundingBox {
            x: 10.0,
            y: 10.0,
            w: 10.0,
            h: 10.0,
        };

        assert!(!intersects_triangle(bbox, (10.0, 10.0)));
        assert!(intersects_triangle(bbox, (20.0, 10.0)));
        assert!(intersects_triangle(bbox, (10.0, 20.0)));
        assert!(intersects_triangle(bbox, (20.0, 20.0)));

        assert!(intersects_triangle(bbox, (15.0, 15.0)));
        assert!(!intersects_triangle(bbox, (14.9, 15.0)));
        assert!(!intersects_triangle(bbox, (15.0, 14.9)));
    }
}
