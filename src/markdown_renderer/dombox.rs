// Copyright 2016 Xavier Bestel -  All rights reserved.
//
// GPL goes here

//! DOM for ANSI terminal rendering

use std::fmt;
use std::borrow::Cow;

use ansi_term::{Style, Colour};
use ansi_term::ANSIString;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

fn findsplit(s: &str, pos: usize) -> usize {
    if let Some(n) = UnicodeSegmentation::grapheme_indices(s, true).nth(pos) {
        return n.0;
    }
    s.len()
}

pub fn split_at_in_place<'a>(cow: &mut Cow<'a, str>, mid: usize) -> Cow<'a, str> {
    match *cow {
        Cow::Owned(ref mut s) => {
            let s2 = s[mid..].to_string();
            s.truncate(mid);
            Cow::Owned(s2)
        }
        Cow::Borrowed(s) => {
            let (s1, s2) = s.split_at(mid);
            *cow = Cow::Borrowed(s1);
            Cow::Borrowed(s2)
        }
    }
}

pub enum TermColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Purple,
    Cyan,
    White,
}

#[derive(Debug, Default, Clone)]
pub struct DomColor(Option<u8>);

impl DomColor {
    pub fn default() -> DomColor {
        DomColor(None)
    }
    pub fn from_dark(color: TermColor) -> DomColor {
        DomColor(Some(color as u8))
    }
    pub fn from_light(color: TermColor) -> DomColor {
        DomColor(Some(color as u8 + 8))
    }
    pub fn from_grey(level: u8) -> DomColor {
        let mut level = level >> 4;
        level = match level {
            0 => 16,
            15 => 231,
            grey => 231 + grey,
        };
        DomColor(Some(level))
    }
    pub fn from_color(red: u8, green: u8, blue: u8) -> DomColor {
        if (red >> 4) == (green >> 4) && (green >> 4) == (blue >> 4) {
            return DomColor::from_grey(red);
        }
        let red = (red as u32 * 6 / 256) as u8;
        let green = (green as u32 * 6 / 256) as u8;
        let blue = (blue as u32 * 6 / 256) as u8;
        DomColor(Some(16 + red * 36 + green * 6 + blue))
    }
    pub fn index(&self) -> Option<u8> {
        self.0
    }
}

#[derive(Debug, Clone)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

impl Default for TextAlign {
    fn default() -> TextAlign {
        TextAlign::Left
    }
}

#[derive(Debug, Copy, Clone)]
pub enum BorderType {
    Empty,
    Dash,
    Thin,
    Double,
    Bold,
}

impl Default for BorderType {
    fn default() -> BorderType {
        BorderType::Empty
    }
}

#[derive(Debug, Default, Clone)]
pub struct DomStyle {
    pub bg: DomColor,
    pub fg: DomColor,
    pub bold: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub italic: bool,
    pub extend: bool,
    pub indent: u16,
    pub align: TextAlign,
    pub border_type: BorderType,
    pub top_nb_type: BorderType,
    pub bottom_nb_type: BorderType,
    pub left_nb_type: BorderType,
    pub right_nb_type: BorderType,
}

impl DomStyle {
    pub fn to_ansi(&self) -> Style {
        let mut astyle = Style::new();
        match self.fg.index() {
            None => {}
            Some(idx) => {
                astyle = astyle.fg(Colour::Fixed(idx));
            }
        }
        match self.bg.index() {
            None => {}
            Some(idx) => {
                astyle = astyle.on(Colour::Fixed(idx));
            }
        }
        if self.bold {
            astyle = astyle.bold();
        }
        if self.underline {
            astyle = astyle.underline();
        }
        if self.strikethrough {
            astyle = astyle.strikethrough();
        }
        if self.italic {
            astyle = astyle.italic();
        }
        astyle
    }

    #[cfg(never)]
    pub fn merge(&mut self, other: DomStyle) -> DomStyle {}
}

#[derive(Debug, Clone)]
pub enum BoxKind<'a> {
    Text(Cow<'a, str>),
    Break,
    InlineContainer,
    Inline,
    Block,
    Header(u8),
    List(Option<u16>),
    ListBullet,
    Table,
    TableColumn,
    TableItem,
    Image,
}

#[derive(Default, Debug, Copy, Clone)]
struct BoxCursor {
    container: BoxSize,
    x: u16,
    y: u16,
}

impl fmt::Display for BoxCursor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{} {}] [{} {} +{} +{}] [+{} +{} -{} -{}]",
            self.x,
            self.y,
            self.container.content.x,
            self.container.content.y,
            self.container.content.w,
            self.container.content.h,
            self.container.border.top,
            self.container.border.left,
            self.container.border.bottom,
            self.container.border.right
        )
    }
}

#[derive(Default, Debug, Copy, Clone)]
pub struct BoxSize {
    pub content: Rect,
    pub border: Edges,
}

impl BoxSize {
    pub fn width_plus_border(&self) -> u16 {
        self.content.w + self.border.left + self.border.right
    }

    pub fn height_plus_border(&self) -> u16 {
        self.content.h + self.border.top + self.border.bottom
    }

    pub fn right(&self) -> u16 {
        self.content.x + self.content.w
    }

    pub fn bottom(&self) -> u16 {
        self.content.y + self.content.h
    }
}

#[derive(Default, Debug, Copy, Clone)]
pub struct Rect {
    x: u16,
    y: u16,
    w: u16,
    h: u16,
}

#[derive(Default, Debug, Copy, Clone)]
pub struct Edges {
    pub top: u16,
    pub bottom: u16,
    pub left: u16,
    pub right: u16,
}

#[derive(Debug)]
enum LayoutRes<T> {
    Normal,
    CutHere(T),
    Reject,
}

#[derive(Debug, Clone)]
pub struct DomBox<'a> {
    pub kind: BoxKind<'a>,
    pub size: BoxSize,
    pub style: DomStyle,
    pub children: Vec<DomBox<'a>>, // TODO no pub => get_children()
}

impl<'a> DomBox<'a> {
    pub fn new_root(width: u16) -> DomBox<'a> {
        let mut dombox = DomBox::new_block();
        dombox.size.content.w = width;
        dombox
    }
    pub fn new_block() -> DomBox<'a> {
        DomBox {
            size: Default::default(),
            kind: BoxKind::Block,
            style: Default::default(),
            children: vec![],
        }
    }
    pub fn swallow(&mut self, existing: DomBox<'a>) {
        self.children.push(existing);
    }
    pub fn get_inline_container(&mut self) -> &mut DomBox<'a> {
        match self.kind {
            BoxKind::Inline | BoxKind::InlineContainer => self,
            _ => {
                match self.children.last() {
                    Some(&DomBox { kind: BoxKind::InlineContainer, .. }) => {}
                    _ => {
                        self.children.push(DomBox {
                            size: Default::default(),
                            kind: BoxKind::InlineContainer,
                            style: self.style.clone(),
                            children: vec![],
                        });
                    }
                }
                self.children.last_mut().unwrap()
            }
        }
    }

    #[cfg(never)]
    pub fn add_dom(&mut self, dom: DomBox<'a>) -> &mut DomBox<'a> {
        if dom.is_inline {
            inline_container = self.get_inline_container();
            inline_container.push(dom);
            inline_container.children.last_mut.clone()
        } else {
            self.children_push(dom);
            self.children.last_mut().unwrap()
        }
    }

    pub fn add_text(&mut self, text: Cow<'a, str>) -> &mut DomBox<'a> {
        let inline_container = self.get_inline_container();
        inline_container.children.push(DomBox {
            size: Default::default(),
            kind: BoxKind::Text(text),
            style: inline_container.style.clone(),
            children: vec![],
        });
        inline_container.children.last_mut().unwrap()
    }
    pub fn add_inline(&mut self) -> &mut DomBox<'a> {
        let inline_container = self.get_inline_container();
        inline_container.children.push(DomBox {
            size: Default::default(),
            kind: BoxKind::Inline,
            style: inline_container.style.clone(),
            children: vec![],
        });
        inline_container.children.last_mut().unwrap()
    }
    pub fn add_block(&mut self) -> &mut DomBox<'a> {
        self.children.push(DomBox {
            size: Default::default(),
            kind: BoxKind::Block,
            style: self.style.clone(),
            children: vec![],
        });
        self.children.last_mut().unwrap()
    }
    pub fn add_header(&mut self, level: u8) -> &mut DomBox<'a> {
        self.children.push(DomBox {
            size: Default::default(),
            kind: BoxKind::Header(level),
            style: self.style.clone(),
            children: vec![],
        });
        self.children.last_mut().unwrap()
    }
    pub fn add_list(&mut self, start: Option<u16>) -> &mut DomBox<'a> {
        self.children.push(DomBox {
            size: Default::default(),
            kind: BoxKind::List(start),
            style: self.style.clone(),
            children: vec![],
        });
        self.children.last_mut().unwrap()
    }
    pub fn add_bullet(&mut self) -> &mut DomBox<'a> {
        self.children.push(DomBox {
            size: Default::default(),
            kind: BoxKind::ListBullet,
            style: self.style.clone(),
            children: vec![],
        });
        self.children.last_mut().unwrap()
    }
    pub fn add_break(&mut self) -> &mut DomBox<'a> {
        self.children.push(DomBox {
            size: Default::default(),
            kind: BoxKind::Break,
            style: self.style.clone(),
            children: vec![],
        });
        self.children.last_mut().unwrap()
    }
    pub fn layout(&mut self) {
        let mut cursor = BoxCursor {
            x: 0,
            y: 0,
            container: self.size,
        };
        self.layout_generic(&mut cursor);
    }
    fn inline_children_loop(
        &mut self,
        res: LayoutRes<DomBox<'a>>,
        dorej: bool,
    ) -> LayoutRes<DomBox<'a>> {
        let mut res = res;
        let mut subcursor = BoxCursor {
            x: self.size.content.x,
            y: self.size.content.y,
            container: self.size,
        };
        let mut i = 0;
        while i < self.children.len() {
            if let BoxKind::Break = self.children[i].kind {
                self.children.remove(i);
                res = LayoutRes::CutHere(DomBox {
                    kind: self.kind.clone(),
                    size: self.size.clone(),
                    style: self.style.clone(),
                    children: self.children.split_off(i),
                });
                break;
            }
            match self.children[i].layout_generic(&mut subcursor) {
                LayoutRes::Normal => (),
                LayoutRes::CutHere(next) => {
                    self.children.insert(i + 1, next);
                    res = LayoutRes::CutHere(DomBox {
                        kind: self.kind.clone(),
                        size: self.size.clone(),
                        style: self.style.clone(),
                        children: self.children.split_off(i + 1),
                    });
                    break;
                }
                LayoutRes::Reject => {
                    if i == 0 {
                        if dorej {
                            res = LayoutRes::Reject;
                        } else {
                            panic!("can't reject from first {:?}", self.children[i].kind);
                        }
                    } else {
                        res = LayoutRes::CutHere(DomBox {
                            kind: self.kind.clone(),
                            size: self.size.clone(),
                            style: self.style.clone(),
                            children: self.children.split_off(i),
                        });
                    }
                    break;
                }
            }
            i += 1;
        }
        self.size.content.w = subcursor.x - self.size.content.x;
        res
    }
    fn layout_generic(&mut self, cursor: &mut BoxCursor) -> LayoutRes<DomBox<'a>> {
        let res = match self.kind {
            BoxKind::Block |
            BoxKind::ListBullet |
            BoxKind::Header(_) => self.layout_block(cursor),
            BoxKind::InlineContainer => self.layout_inline_container(cursor),
            BoxKind::List(_) => self.layout_list(cursor),
            BoxKind::Text(_) | BoxKind::Inline => self.layout_inline(cursor),
            BoxKind::Break => panic!("shouldn't layout a break"),
            _ => panic!("unimplemented layout for {:?}", self.kind),
        };
        res
    }
    fn layout_block(&mut self, cursor: &mut BoxCursor) -> LayoutRes<DomBox<'a>> {
        let res = LayoutRes::Normal;
        self.size.content.x = cursor.x + self.size.border.left;
        self.size.content.y = cursor.y + self.size.border.top;
        self.size.content.h = 0;
        self.size.content.w = if cursor.container.content.w - cursor.x +
            cursor.container.content.x >
            self.size.border.left + self.size.border.right
        {
            cursor.container.content.w - cursor.x + cursor.container.content.x -
                self.size.border.left - self.size.border.right
        } else {
            1
        };
        let mut subcursor = BoxCursor {
            x: self.size.content.x,
            y: self.size.content.y,
            container: self.size,
        };
        let mut max_width = 0;
        let mut i = 0;
        while i < self.children.len() {
            if let BoxKind::Break = self.children[i].kind {
                self.children.remove(i);
                continue;
            }

            self.layout_child(&mut subcursor, i);

            self.size.content.h += self.children[i].size.height_plus_border();

            if self.children[i].size.width_plus_border() > max_width {
                max_width = self.children[i].size.width_plus_border();
            }

            i += 1;
        }
        if !self.style.extend {
            self.size.content.w = max_width;
        }
        if let BoxKind::ListBullet = self.kind {
            // XXX ugly
            cursor.x += self.size.width_plus_border();
        } else {
            cursor.x = cursor.container.content.x;
            cursor.y += self.size.height_plus_border();
        }

        res
    }

    fn layout_child(&mut self, cursor: &mut BoxCursor, i: usize) {
        match self.children[i].layout_generic(cursor) {
            LayoutRes::Normal => (),
            LayoutRes::CutHere(next) => self.children.insert(i + 1, next),
            LayoutRes::Reject => {
                panic!("can't reject a {:?}", self.children[i].kind);
            }
        }
    }

    fn layout_list(&mut self, cursor: &mut BoxCursor) -> LayoutRes<DomBox<'a>> {
        let res = LayoutRes::Normal;
        self.size.content.w =
            if cursor.container.content.w > self.size.border.left + self.size.border.right {
                cursor.container.content.w - self.size.border.left - self.size.border.right
            } else {
                1
            };
        self.size.content.h = 0;
        self.size.content.x = cursor.x + self.size.border.left;
        self.size.content.y = cursor.y + self.size.border.top;
        let mut subcursor = BoxCursor {
            x: self.size.content.x,
            y: self.size.content.y,
            container: self.size,
        };
        let mut i = 0;
        while i < self.children.len() {
            self.layout_child(&mut subcursor, i);
            match self.children[i].kind {
                BoxKind::ListBullet => (),
                BoxKind::Block => {
                    self.size.content.h += self.children[i].size.height_plus_border();
                }
                _ => panic!("can't layout a {:?} in a List", self.children[i].kind),
            }
            i += 1;
        }
        cursor.y += self.size.height_plus_border();
        res
    }
    // this is a line, and when split will be 2 lines
    fn layout_inline_container(&mut self, cursor: &mut BoxCursor) -> LayoutRes<DomBox<'a>> {
        let mut res = LayoutRes::Normal;
        self.size.content.w =
            if cursor.container.content.w > self.size.border.left + self.size.border.right {
                cursor.container.content.w - self.size.border.left - self.size.border.right
            } else {
                1
            };
        self.size.content.h = 1;
        self.size.content.x = cursor.x + self.size.border.left;
        self.size.content.y = cursor.y + self.size.border.top;
        res = self.inline_children_loop(res, false);
        cursor.y += self.size.height_plus_border();
        res
    }
    // this one can ask to be splitted if needs be, in this case the returned
    // element must be inserted right after the current one
    fn layout_inline(&mut self, cursor: &mut BoxCursor) -> LayoutRes<DomBox<'a>> {
        let mut res = LayoutRes::Normal;
        self.size.content.h = 1;
        self.size.content.x = cursor.x + self.size.border.left;
        self.size.content.y = cursor.y + self.size.border.top;
        self.size.content.w = cursor.container.content.w - (cursor.x - cursor.container.content.x) -
            (self.size.border.left + self.size.border.right);
        match self.kind {
            BoxKind::Text(ref mut text) => {
                let width = UnicodeWidthStr::width(&text[..]) as u16;
                if self.size.content.w == 0 {
                    res = LayoutRes::Reject;
                } else if width > self.size.content.w {
                    let pos = findsplit(text, self.size.content.w as usize);
                    let remains = split_at_in_place(text, pos);
                    res = LayoutRes::CutHere(DomBox {
                        kind: BoxKind::Text(remains),
                        size: self.size.clone(),
                        style: self.style.clone(),
                        children: vec![],
                    });
                } else {
                    self.size.content.w = width;
                }
            }
            BoxKind::Inline => {
                res = self.inline_children_loop(res, true);
            }
            _ => {
                panic!("can't layout_inline {:?}", self.kind);
            }
        };
        cursor.x += self.size.content.w;
        res
    }

    pub fn render(&mut self) -> Vec<ANSIString<'a>> {
        let mut strings = Vec::new();
        for line in 0..(self.size.height_plus_border()) {
            self.render_line(line, &mut strings);
            strings.push(Style::default().paint("\n"));
        }

        strings
    }

    fn render_line(&self, line: u16, strings: &mut Vec<ANSIString<'a>>) -> (u16, u16) {
        if line < self.size.content.y - self.size.border.top ||
            line >= self.size.bottom() + self.size.border.bottom
        {
            // out of the box, don't render anything
            return (0, 0);
        }
        if line < self.size.content.y || line >= self.size.bottom() {
            return self.render_borderline(line, strings);
        }
        self.render_borderside(true, strings);
        let mut pos = self.size.content.x;
        match self.kind {
            BoxKind::Text(ref text) => {
                let s = self.style.to_ansi().paint(text.to_string());
                strings.push(s);
                pos += UnicodeWidthStr::width(&text[..]) as u16;
                assert!(pos <= self.size.right());
            }
            _ => {
                for child in &self.children {
                    let insert_point = strings.len() as u16;
                    let (start, len) = child.render_line(line, strings);
                    if len == 0 {
                        continue;
                    }
                    assert!(start >= pos);
                    assert!(start + len <= self.size.right());
                    if start > pos {
                        self.render_charline(' ', start - pos, Some(insert_point), strings);
                    }
                    pos = start + len;
                }
                assert!(pos <= self.size.right());
            }
        }
        if pos < self.size.right() {
            self.render_charline(' ', self.size.right() - pos, None, strings);
        }
        self.render_borderside(false, strings);
        return (
            self.size.content.x - self.size.border.left,
            self.size.width_plus_border(),
        );
    }
    fn render_borderline(&self, line: u16, strings: &mut Vec<ANSIString<'a>>) -> (u16, u16) {
        let is_top = line < self.size.content.y;
        let mut s = String::with_capacity(((self.size.width_plus_border()) * 4) as usize);
        for _ in 0..self.size.border.left {
            match self.style.border_type {
                _ => {
                    s.push(if is_top { '┌' } else { '└' });
                }
            }
        }
        for _ in 0..self.size.content.w {
            match self.style.border_type {
                BorderType::Empty => {
                    s.push(' ');
                }
                BorderType::Dash => {
                    s.push('╌');
                }
                BorderType::Thin => {
                    s.push('─');
                }
                BorderType::Double => {
                    s.push('═');
                }
                BorderType::Bold => {
                    s.push('━');
                }
            }
        }
        for _ in 0..self.size.border.right {
            s.push(if is_top { '┐' } else { '┘' });
        }
        let s = self.style.to_ansi().paint(s);
        strings.push(s);
        return (
            self.size.content.x - self.size.border.left,
            self.size.width_plus_border(),
        );
    }
    fn render_borderside(&self, is_left: bool, strings: &mut Vec<ANSIString<'a>>) {
        let width = if is_left {
            self.size.border.left
        } else {
            self.size.border.right
        };
        let mut s = String::with_capacity((width * 4) as usize);
        for _ in 0..width {
            match self.style.border_type {
                BorderType::Empty => {
                    s.push(' ');
                }
                BorderType::Dash => {
                    s.push('╎');
                }
                BorderType::Thin => {
                    s.push('│');
                }
                BorderType::Double => {
                    s.push('║');
                }
                BorderType::Bold => {
                    s.push('┃');
                }
            }
        }
        let s = self.style.to_ansi().paint(s);
        strings.push(s);
    }
    fn render_charline(
        &self,
        c: char,
        n: u16,
        insert: Option<u16>,
        strings: &mut Vec<ANSIString<'a>>,
    ) {
        let mut s = String::with_capacity((n * 4) as usize);
        for _ in 0..n {
            s.push(c);
        }
        let s = self.style.to_ansi().paint(s);
        if let Some(insert) = insert {
            strings.insert(insert as usize, s);
        } else {
            strings.push(s);
        }
    }
}
