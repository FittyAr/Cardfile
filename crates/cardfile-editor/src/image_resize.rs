//! Resizing images with handles: the corner keeps the aspect ratio, the right and
//! bottom edges stretch freely. The new size is previewed while dragging and
//! committed as one undoable edit on mouse up.

use cardfile_doc::{Block, BlockKind};
use gpui_kit::{Bounds, Context, MouseMoveEvent, Pixels, Point, px};

use crate::editor::RichEditor;
use crate::history::EditKind;

/// Which handle is being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// Bottom-right corner: width changes, height follows the aspect ratio.
    Corner,
    /// Right edge: width only.
    Right,
    /// Bottom edge: height only.
    Bottom,
}

#[derive(Clone, Debug)]
pub struct ImageDrag {
    pub block: usize,
    pub handle: Handle,
    start: Point<Pixels>,
    start_w: f32,
    start_h: f32,
    /// Live size shown while dragging.
    pub width: f32,
    pub height: Option<f32>,
}

const MIN: f32 = 24.;
const MAX: f32 = 4000.;

/// The width/height the image block shows, honouring a resize in progress.
pub fn preview_size(drag: Option<&ImageDrag>, block: usize) -> Option<(f32, Option<f32>)> {
    drag.filter(|d| d.block == block)
        .map(|d| (d.width, d.height))
}

impl RichEditor {
    /// Start dragging `handle` of the image in `block`.
    pub(crate) fn start_image_drag(
        &mut self,
        block: usize,
        handle: Handle,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.is_read_only() {
            return;
        }
        let Some(bounds): Option<Bounds<Pixels>> = self.image_bounds.borrow().get(&block).copied()
        else {
            return;
        };
        let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let stretched = matches!(
            self.model.doc.blocks.get(block),
            Some(Block { kind: BlockKind::Image(img), .. }) if img.height.is_some()
        );
        self.image_drag = Some(ImageDrag {
            block,
            handle,
            start: at,
            start_w: w,
            start_h: h,
            width: w,
            height: (stretched || handle != Handle::Corner).then_some(h),
        });
        cx.notify();
    }

    pub(crate) fn update_image_drag(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(drag) = self.image_drag.as_mut() else {
            return;
        };
        let dx = f32::from(ev.position.x - drag.start.x);
        let dy = f32::from(ev.position.y - drag.start.y);
        match drag.handle {
            Handle::Corner => {
                drag.width = (drag.start_w + dx).clamp(MIN, MAX);
                // Corner keeps proportions: drop any stretch.
                drag.height = None;
            }
            Handle::Right => {
                drag.width = (drag.start_w + dx).clamp(MIN, MAX);
                drag.height = Some(drag.start_h);
            }
            Handle::Bottom => {
                drag.width = drag.start_w;
                drag.height = Some((drag.start_h + dy).clamp(MIN, MAX));
            }
        }
        cx.notify();
    }

    pub(crate) fn finish_image_drag(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.image_drag.take() else {
            return;
        };
        self.edit(EditKind::Other, cx, |m| {
            if let Some(Block {
                kind: BlockKind::Image(img),
                ..
            }) = m.doc.blocks.get_mut(drag.block)
            {
                img.width = Some(drag.width.round());
                img.height = drag.height.map(f32::round);
            }
        });
        cx.notify();
    }
}

/// Size of a handle square.
pub const HANDLE: Pixels = px(10.);
