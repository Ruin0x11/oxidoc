use ::store;

trait DocPart {
    fn render(&self) -> String;
}

struct Document {
    parts: Vec<DocPart>,
}

impl Document {
    fn display(&self) {
        
    }

    fn add_method(&mut self, name: String) {
        let result = store::lookup_method(name);

        
    }
}
