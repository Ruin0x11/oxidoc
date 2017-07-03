// Copyright 2016 Xavier Bestel -  All rights reserved.
//
// GPL goes here

//! ANSI renderer for pulldown-cmark.

use std::borrow::Cow;

use html2runes;
use pulldown_cmark::{Event, Tag};
use pulldown_cmark::Event::{Start, End, Text, Html, InlineHtml, SoftBreak, HardBreak,
                            FootnoteReference};

use syntect::easy::HighlightLines;
use syntect::parsing::SyntaxSet;
use syntect::highlighting;
use syntect::parsing::syntax_definition::SyntaxDefinition;

use super::dombox::{DomBox, BorderType, DomColor, TermColor, BoxKind, split_at_in_place};

struct Ctx<'a, 'b, I> {
    iter: I,
    links: Option<DomBox<'a>>,
    footnotes: Option<DomBox<'a>>,
    syntaxes: &'b SyntaxSet,
    themes: &'b highlighting::ThemeSet,
    syntax: Option<&'b SyntaxDefinition>,
    pub theme: &'b str,
    highline: Option<HighlightLines<'b>>,
}

impl<'a, 'b, I: Iterator<Item = Event<'a>>> Ctx<'a, 'b, I> {
    pub fn new(iter: I, syntaxes: &'b SyntaxSet, themes: &'b highlighting::ThemeSet) -> Self {
        Ctx {
            iter: iter,
            links: None,
            footnotes: None,
            syntaxes: syntaxes,
            themes: themes,
            syntax: None,
            theme: "base16-eighties.dark",
            highline: None,
        }
    }
    fn build(&mut self, width: u16) -> DomBox<'a> {
        self.links = Some(DomBox::new_block());
        self.footnotes = Some(DomBox::new_block());
        let mut root = DomBox::new_root(width);
        self.build_dom(&mut root);
        if let Some(links) = self.links.take() {
            root.swallow(links);
        }
        if let Some(footnotes) = self.footnotes.take() {
            root.swallow(footnotes);
        }
        root
    }
    fn build_dom(&mut self, parent: &mut DomBox<'a>) {
        loop {
            match self.iter.next() {
                Some(event) => {
                    match event {
                        Start(tag) => {
                            match tag {
                                Tag::Paragraph => {
                                    let child = parent.add_block();
                                    self.build_dom(child);
                                    child.size.border.bottom = 1;
                                }
                                Tag::Rule => {
                                    let child = parent.add_block();
                                    child.style.extend = true;
                                    child.size.border.bottom = 1;
                                    child.style.border_type = BorderType::Thin;
                                    child.style.fg = DomColor::from_dark(TermColor::Yellow);
                                }
                                Tag::Header(level) => {
                                    let child = parent.add_header(level as u8);
                                    child.size.border.bottom = 1;
                                    match level {
                                        1 => {
                                            child.size.border.top = 1;
                                            child.size.border.left = 1;
                                            child.size.border.right = 1;
                                            child.style.border_type = BorderType::Thin;
                                        }
                                        2 => {
                                            child.style.border_type = BorderType::Bold;
                                        }
                                        3 => {
                                            child.style.border_type = BorderType::Double;
                                        }
                                        4 => {
                                            child.style.border_type = BorderType::Thin;
                                        }
                                        5 => {
                                            child.style.border_type = BorderType::Dash;
                                        }
                                        6 => {}
                                        bad => panic!("wrong heading size {}", bad),
                                    }
                                    child.style.fg = DomColor::from_dark(TermColor::Purple);
                                    self.build_dom(child);
                                }
                                Tag::Table(_) => {}
                                Tag::TableHead => {}
                                Tag::TableRow => {}
                                Tag::TableCell => {}
                                Tag::BlockQuote => {
                                    {
                                        let child = parent.add_block();
                                        self.build_dom(child);
                                        child.size.border.left = 1;
                                        child.style.border_type = BorderType::Thin;
                                        child.style.fg = DomColor::from_dark(TermColor::Cyan);
                                    }
                                    let newline = parent.add_block(); // XXX ugly
                                    newline.add_text(Cow::from(""));
                                }
                                Tag::CodeBlock(info) => {
                                    {
                                        let indent = parent.style.indent;
                                        let child = parent.add_block();
                                        child.style.code = true;
                                        child.style.fg = DomColor::from_dark(TermColor::White);
                                        child.style.bg = DomColor::from_dark(TermColor::Black);
                                        child.style.indent = indent + 2;

                                        // NOTE: Just assume the language is rust if the language
                                        // is omitted, since many docs don't have the 'rust' tag in
                                        // code blocks
                                        self.syntax = match self.syntaxes.find_syntax_by_token(&info) {
                                            Some(syn) => Some(syn),
                                            None => self.syntaxes.find_syntax_by_token("rust"),
                                        };

                                        if let Some(syn) = self.syntax {
                                            self.highline = Some(HighlightLines::new(
                                                syn,
                                                &self.themes.themes[self.theme],
                                            ));

                                            self.build_dom(child);
                                        }
                                    }
                                    let newline = parent.add_block(); // XXX ugly
                                    newline.add_text(Cow::from(""));
                                }
                                Tag::List(Some(start)) => {
                                    let child = parent.add_list(Some(start as u16));
                                    self.build_dom(child);
                                    child.size.border.bottom = 1;
                                }
                                Tag::List(None) => {
                                    let child = parent.add_list(None);
                                    self.build_dom(child);
                                    child.size.border.bottom = 1;
                                }
                                Tag::Item => {
                                    {
                                        let bullet = parent.add_bullet();
                                        bullet.style.fg = DomColor::from_light(TermColor::Yellow);
                                        bullet.size.border.right = 1;
                                    }
                                    let child = parent.add_block();
                                    self.build_dom(child);
                                }
                                Tag::Emphasis => {
                                    let child = parent.add_inline();
                                    child.style.italic = true;
                                    self.build_dom(child);
                                }
                                Tag::Strong => {
                                    let child = parent.add_inline();
                                    child.style.bold = true;
                                    self.build_dom(child);
                                }
                                Tag::Code => {
                                    let child = parent.add_inline();
                                    child.style.code = true;
                                    child.style.fg = DomColor::from_dark(TermColor::White);
                                    child.style.bg = DomColor::from_dark(TermColor::Black);
                                    self.build_dom(child);
                                }
                                Tag::Link(dest, _) => {
                                    if let Some(mut links) = self.links.take() {
                                        {
                                            let child = links.add_text(dest);
                                            child.style.fg = DomColor::from_dark(TermColor::Blue);
                                            child.style.underline = true;
                                        }
                                        {
                                            links.add_break();
                                        }
                                        self.links = Some(links);
                                    }
                                    let child = parent.add_inline();
                                    child.style.underline = true;
                                    child.style.fg = DomColor::from_dark(TermColor::Blue);
                                    self.build_dom(child);
                                }
                                Tag::Image(dest, title) => {
                                    {
                                        let child = parent.add_text(title);
                                        child.style.fg = DomColor::from_light(TermColor::Black);
                                        child.style.bg = DomColor::from_dark(TermColor::Yellow);
                                    }
                                    {
                                        let child = parent.add_text(dest);
                                        child.style.fg = DomColor::from_dark(TermColor::Blue);
                                        child.style.bg = DomColor::from_dark(TermColor::Yellow);
                                        child.style.underline = true;
                                    }
                                    let child = parent.add_inline();
                                    child.style.italic = true;
                                    self.build_dom(child);
                                }
                                Tag::FootnoteDefinition(name) => {
                                    if let Some(mut footnotes) = self.footnotes.take() {
                                        {
                                            let child = footnotes.add_text(name);
                                            child.style.fg = DomColor::from_dark(TermColor::Green);
                                            child.style.underline = true;
                                        }
                                        self.build_dom(&mut footnotes);
                                        self.footnotes = Some(footnotes);
                                    }
                                }
                            }
                        }
                        End(tag) => {
                            match tag {
                                Tag::Paragraph => {
                                    break;
                                }
                                Tag::Rule => {}
                                Tag::Header(_) => {
                                    break;
                                }
                                Tag::Table(_) => {}
                                Tag::TableHead => {}
                                Tag::TableRow => {}
                                Tag::TableCell => {}
                                Tag::BlockQuote => {
                                    break;
                                }
                                Tag::CodeBlock(_) => {
                                    self.highline = None;
                                    self.syntax = None;
                                    break;
                                }
                                Tag::List(None) => {
                                    for child in &mut parent.children {
                                        {
                                            if let BoxKind::ListBullet = child.kind {
                                                child.add_text(Cow::from("*"));
                                            }
                                        }
                                    }
                                    break;
                                }
                                Tag::List(Some(start)) => {
                                    let mut i = start;
                                    // TODO resize all bullets like the last one
                                    //let end = start + node.children.len() / 2;
                                    for child in &mut parent.children {
                                        {
                                            if let BoxKind::ListBullet = child.kind {
                                                child.add_text(Cow::from(i.to_string()));
                                                i += 1;
                                            }
                                        }
                                    }
                                    break;
                                }
                                Tag::Item => {
                                    break;
                                }
                                Tag::Emphasis => {
                                    break;
                                }
                                Tag::Strong => {
                                    break;
                                }
                                Tag::Code => {
                                    break;
                                }
                                Tag::Link(_, _) => {
                                    break;
                                }
                                Tag::Image(_, _) => {
                                    break;
                                }
                                Tag::FootnoteDefinition(_) => {
                                    break;
                                }
                            }
                        }
                        Text(mut text) => {
                            if let Some(ref mut h) = self.highline {
                                match text {
                                    Cow::Borrowed(text) => {
                                        let ranges = h.highlight(&text);
                                        for (style, mut text) in ranges {
                                            let mut add_break = false;
                                            if text.len() > 0 {
                                                // check if text ends with a newline
                                                let bytes = text.as_bytes();
                                                if bytes[bytes.len() - 1] == 10 {
                                                    add_break = true;
                                                }
                                            }
                                            if add_break {
                                                text = &text[..text.len() - 1];
                                            }
                                            {
                                                let child = parent.add_text(Cow::Borrowed(text));
                                                child.style.fg = DomColor::from_color(
                                                    style.foreground.r,
                                                    style.foreground.g,
                                                    style.foreground.b,
                                                );
                                                child.style.bold |= style.font_style.intersects(
                                                    highlighting::FONT_STYLE_BOLD,
                                                );
                                                child.style.italic |= style.font_style.intersects(
                                                    highlighting::FONT_STYLE_ITALIC,
                                                );
                                                child.style.underline |=
                                                    style.font_style.intersects(
                                                        highlighting::FONT_STYLE_UNDERLINE,
                                                    );
                                            }
                                            if add_break {
                                                parent.add_break();
                                            }
                                        }
                                    }
                                    Cow::Owned(_text) => {
                                        unimplemented!();
                                    }
                                }
                            } else {
                                let mut add_break = false;
                                if text.len() > 0 {
                                    // check if text ends with a newline
                                    let bytes = text.as_bytes();
                                    if bytes[bytes.len() - 1] == 10 {
                                        add_break = true;
                                    }
                                }
                                if add_break {
                                    let pos = text.len() - 1;
                                    split_at_in_place(&mut text, pos);
                                }
                                parent.add_text(text);
                                if add_break {
                                    parent.add_break();
                                }
                            }
                        }
                        Html(html) | InlineHtml(html) => {
                            let text = html2runes::html_to_text(&html.clone().to_mut());
                            let child = parent.add_text(Cow::from(text));
                            child.style.fg = DomColor::from_light(TermColor::Red);
                        }
                        SoftBreak => {
                            parent.add_break();
                        }
                        HardBreak => {
                            parent.add_break();
                        }
                        FootnoteReference(name) => {
                            let child = parent.add_text(name);
                            child.style.fg = DomColor::from_dark(TermColor::Green);
                            child.style.underline = true;
                        }
                    }
                }
                None => break,
            }
        }
    }
}

pub fn push_ansi<'a, I: Iterator<Item = Event<'a>>>(iter: I, width: u16) -> String {
    let syntaxes = SyntaxSet::load_defaults_newlines();
    let themes = highlighting::ThemeSet::load_defaults();
    let mut ctx = Ctx::new(iter, &syntaxes, &themes);
    let mut root = ctx.build(width);
    root.layout();
    let ansi_strings = root.render();

    ansi_strings.into_iter().fold(String::new(), |s, ansi| s + &ansi.to_string())
}
