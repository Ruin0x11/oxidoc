use std::fmt;

use ansi_term::Style;
use catmark::{self, OutputKind};
use convert::*;
use term_size;

use document::{Attributes, FnKind, ModPath};
use driver::Driver;

pub enum Markup {
    Header(String),
    Section(String),
    Block(String),
    Markdown(String),
    Rule(usize),
    LineBreak,
}

use self::Markup::*;

fn get_term_width() -> u16 {
    match term_size::dimensions() {
        Some((w, _)) => w as u16,
        None => 80,
    }
}

impl fmt::Display for Markup {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let string = match *self {
            Header(ref text) => {
                Style::new()
                    .bold()
                    .paint(format!("==== {}", text))
                    .to_string()
            }
            Section(ref text) => {
                Style::new()
                    .bold()
                    .paint(format!("== {}", text))
                    .to_string()
            }
            Block(ref text) => text.clone(),
            Markdown(ref md) => {
                let width = get_term_width();
                catmark::render_ansi(md, width, OutputKind::Color)
            }
            Rule(ref count) => "-".repeat(*count),
            LineBreak => "".to_string(),
        };
        write!(f, "{}", string)
    }
}

pub struct MarkupDoc {
    pub parts: Vec<Markup>,
}

impl MarkupDoc {
    pub fn new(parts: Vec<Markup>) -> Self {
        MarkupDoc { parts: parts }
    }
}

impl fmt::Display for MarkupDoc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for part in self.parts.iter() {
            part.fmt(f)?;
            write!(f, "\n")?;
        }
        Ok(())
    }
}

pub trait Format {
    fn format(&self) -> MarkupDoc;
}

impl Format for Documentation {
    fn format(&self) -> MarkupDoc {
        let header = doc_header(self);
        let info = doc_inner_info(self);
        let signature = doc_signature(self);
        let body = doc_body(self);
        let related_items = doc_related_items(self);

        let mut result = Vec::new();
        result.extend(header.parts);
        result.extend(info.parts);
        result.extend(signature.parts);
        result.extend(body.parts);
        result.extend(related_items.parts);

        MarkupDoc::new(result)
    }
}

impl Format for ModPath {
    fn format(&self) -> MarkupDoc {
        MarkupDoc::new(vec![Header(self.to_string())])
    }
}

impl Format for Attributes {
    fn format(&self) -> MarkupDoc {
        let body = self.doc_strings.join("\n");

        MarkupDoc::new(vec![Markdown(body)])
    }
}

fn doc_header(doc: &Documentation) -> MarkupDoc {
    let name = match doc.inner_data {
        DocInnerData::FnDoc(..) => "Function",
        DocInnerData::StructDoc(..) => "Struct",
        DocInnerData::ConstDoc(..) => "Constant",
        DocInnerData::EnumDoc(..) => "Enum",
        DocInnerData::TraitDoc(..) => "Trait",
        DocInnerData::TraitItemDoc(..) => "Trait Item",
        DocInnerData::ModuleDoc(ref module) => if module.is_crate { "Crate" } else { "Module" },
    };

    MarkupDoc::new(vec![
        Block(format!("({})", doc.crate_info)),
        Header(format!("{} {}", name, doc.mod_path)),
    ])
}

fn doc_inner_info(doc: &Documentation) -> MarkupDoc {
    let markup = match doc.inner_data {
        DocInnerData::FnDoc(ref func) => {
            match func.kind {
                FnKind::MethodFromImpl => Header(format!(
                    "Impl on type {}",
                    doc.mod_path.parent().unwrap()
                )),
                _ => LineBreak,
            }
        }
        DocInnerData::TraitItemDoc(..) => {
            Header(format!("From trait {}", doc.mod_path.parent().unwrap()))
        }
        DocInnerData::StructDoc(..) |
        DocInnerData::ConstDoc(..) |
        DocInnerData::EnumDoc(..) |
        DocInnerData::TraitDoc(..) |
        DocInnerData::ModuleDoc(..) => LineBreak,
    };
    MarkupDoc::new(vec![markup])
}

fn header_string(doc: &Documentation) -> String {
    match doc.inner_data {
        DocInnerData::ModuleDoc(..) => format!("mod {}", doc.mod_path),
        DocInnerData::FnDoc(ref func) => format!("fn {} {}", doc.name, func.header),
        DocInnerData::EnumDoc(..) => format!("enum {}", doc.name),
        DocInnerData::StructDoc(..) => format!("struct {} {{ /* fields omitted */ }}", doc.name),
        DocInnerData::ConstDoc(ref const_) => {
            format!("const {}: {} = {}", doc.name, const_.ty.name, const_.expr)
        }
        DocInnerData::TraitDoc(..) => format!("trait {} {{ /* fields omitted */ }}", doc.name),
        DocInnerData::TraitItemDoc(ref item) => format!("{}", trait_item(doc, item)),
    }
}

fn doc_signature(doc: &Documentation) -> MarkupDoc {
    if let DocInnerData::ModuleDoc(ref module) = doc.inner_data {
        if module.is_crate {
            return MarkupDoc::new(vec![Rule(10), LineBreak]);
        }
    }

    let vis_string = match doc.visibility {
        Some(ref v) => v.to_string(),
        None => "".to_string(),
    };

    let header = header_string(doc);

    MarkupDoc::new(vec![
        Rule(10),
        LineBreak,
        Block(format!("  {} {}", vis_string, header)),
        LineBreak,
        Rule(10),
        LineBreak,
    ])
}

fn trait_item(doc: &Documentation, item: &TraitItem) -> String {
    let item_string = match item.node {
        TraitItemKind::Const(ref ty, ref expr) => {
            let expr_string = match *expr {
                Some(ref e) => e.clone(),
                None => "".to_string(),
            };
            format!("const {}: {} = {}", doc.name, ty.name, expr_string)
        }
        TraitItemKind::Method(ref sig) => format!("fn {} {}", doc.name, sig.header),
        TraitItemKind::Type(ref ty) => {
            let ty_string = match *ty {
                Some(ref t) => t.name.clone(),
                None => "".to_string(),
            };
            format!("type {}", ty_string)
        }
        TraitItemKind::Macro(ref mac) => format!("macro {} {}", doc.name, mac),
    };
    item_string
}

fn doc_body(doc: &Documentation) -> MarkupDoc {
    doc.attrs.format()
}

fn related_item(item: &Documentation) -> MarkupDoc {
    let header = header_string(item);

    let mut markup = vec![Header(header), LineBreak];

    markup.extend(doc_body(item).parts);

    MarkupDoc::new(markup)
}

fn doc_related_items(doc: &Documentation) -> MarkupDoc {
    let mut markup = vec![];
    for (type_, links) in doc.links.iter() {
        markup.push(Section(type_.to_string()));
        for link in links.iter() {
            let item = Driver::get_doc(link);
            let doc = match item {
                Ok(i) => related_item(&i),
                Err(e) => MarkupDoc::new(vec![Block(e.to_string()), Block(format!("{:?}", link))]),
            };
            markup.extend(doc.parts);
        }
    }

    MarkupDoc::new(markup)
}
