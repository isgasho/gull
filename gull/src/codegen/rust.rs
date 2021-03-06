use super::docs::{format_docstring, CommentStyle};
use super::Codegen;
use crate::definitions::*;
use anyhow::Result;
use std::cell::RefCell;
use std::collections::BTreeSet;

pub struct RustCodegen {
    imports: RefCell<BTreeSet<&'static str>>,
}

impl Codegen for RustCodegen {
    fn gen_declarations(declarations: &Declarations) -> Result<String> {
        let rc = RustCodegen::new();

        let mut declarations_code = String::new();

        for declaration in &declarations.declarations {
            declarations_code.push('\n');
            declarations_code.push_str(&rc.gen_declaration(declaration)?);
            declarations_code.push('\n');
        }

        let mut result = String::new();

        for import in rc.imports.borrow().iter() {
            result.push_str(&format!("{}\n", import));
        }

        result.push('\n');
        result.push_str(&declarations_code);

        Ok(result)
    }
}

impl RustCodegen {
    fn new() -> Self {
        Self {
            imports: RefCell::new(BTreeSet::new()),
        }
    }

    fn add_import(&self, import: &'static str) {
        self.imports.borrow_mut().insert(import);
    }

    fn gen_declaration(&self, declaration: &TypeDeclaration) -> Result<String> {
        let mut prefix = String::new();
        for config in &declaration.config {
            match config {
                TypeDeclarationConfig::RustAttribute(attr) => {
                    prefix.push_str(attr);
                    prefix.push('\n')
                }
            }
        }

        let mut r = match &declaration.value {
            DeclarationValue::TPrimitive(p) => format!(
                "pub type {} = {};",
                declaration.name,
                self.gen_primitive_type(p)
            )
            .into(),
            DeclarationValue::TMap(m) => {
                format!("pub type {} = {};", declaration.name, self.gen_map(m))
            }
            DeclarationValue::TTuple(t) => {
                format!("pub type {} = {};", declaration.name, self.gen_tuple(t))
            }
            DeclarationValue::TStruct(s) => {
                prefix.push_str("#[derive(Debug, serde::Serialize, serde::Deserialize)]\n");
                format!("pub struct {} {}", declaration.name, self.gen_struct(s, 0))
            }
            DeclarationValue::TEnum(e) => {
                format!("pub enum {} {}", declaration.name, self.gen_enum(e))
            }
            DeclarationValue::Docs => String::new(),
        };

        let comment_style = if let DeclarationValue::Docs = declaration.value {
            CommentStyle::DoubleSlash
        } else {
            CommentStyle::TripleSlash
        };

        if let Some(doc) = format_docstring(declaration.docs, comment_style, 0) {
            r = format!("{}\n{}", doc, r);
        }

        Ok(format!("{}{}", prefix, r))
    }

    fn gen_map(&self, m: &TMap) -> String {
        let value = match &m.value {
            TMapValue::TPrimitive(p) => self.gen_primitive_type(p),
            TMapValue::Reference(d) => d.name,
        };

        self.add_import("use std::collections::BTreeMap;");
        format!("BTreeMap<{}, {}>", self.gen_primitive_type(&m.key), value)
    }

    fn gen_vec(&self, v: &TVec) -> String {
        let value = match &v {
            TVec::TPrimitive(p) => self.gen_primitive_type(p),
            TVec::Reference(d) => d.name,
        };
        format!("Vec<{}>", value)
    }

    fn gen_set(&self, s: &TSet) -> String {
        let value = match &s {
            TSet::TPrimitive(p) => self.gen_primitive_type(p),
            TSet::Reference(d) => d.name,
        };

        self.add_import("use std::collections::BTreeSet;");
        format!("BTreeSet<{}>", value)
    }

    fn gen_option(&self, o: &TOption) -> String {
        let value = match &o {
            TOption::Reference(r) => r.name.into(),
            TOption::TPrimitive(p) => self.gen_primitive_type(&p).into(),
            TOption::TMap(m) => self.gen_map(m),
            TOption::TVec(v) => self.gen_vec(v),
            TOption::TSet(s) => self.gen_set(s),
        };
        format!("Option<{}>", value)
    }

    fn gen_struct(&self, s: &TStruct, indent_level: usize) -> String {
        let mut fields = String::new();

        let indent = " ".repeat(indent_level);

        for field in s.fields.iter() {
            let mut field_prefix = String::new();

            for config in &field.config {
                match config {
                    StructFieldConfig::RustAttribute(attr) => {
                        field_prefix.push_str(&format!("\n    {}{}", indent, attr))
                    }
                }
            }

            let field_type = match &field.field_type {
                StructFieldType::Reference(r) => r.name.into(),
                StructFieldType::TMap(m) => self.gen_map(m),
                StructFieldType::TOption(o) => self.gen_option(o),
                StructFieldType::TPrimitive(p) => self.gen_primitive_type(&p).into(),
                StructFieldType::TTuple(t) => self.gen_tuple(t),
                StructFieldType::TVec(v) => self.gen_vec(v),
            };

            let mut field_str = format!("\n    {}{}: {},", &indent, field.name, field_type);

            if let Some(doc) =
                format_docstring(field.docs, CommentStyle::TripleSlash, indent_level + 4)
            {
                field_str = format!("\n{}{}", doc, field_str);
            }

            fields.push_str(&format!("{}{}", field_prefix, field_str));
        }

        format!("{{{}\n{}}}", fields, indent)
    }

    fn gen_enum(&self, e: &TEnum) -> String {
        let mut variants = String::new();

        for variant in &e.variants {
            let mut variant_type = match &variant.variant_type {
                EnumVariantType::Empty => "".into(),
                EnumVariantType::Tuple(t) => self.gen_tuple(t),
                EnumVariantType::Struct(s) => format!(" {}", self.gen_struct(s, 4)),
            };

            variant_type = format!("\n    {}{},", variant.name, variant_type);
            if let Some(doc) = format_docstring(variant.docs, CommentStyle::TripleSlash, 4) {
                variant_type = format!("\n{}{}", doc, variant_type);
            }

            variants.push_str(&variant_type);
        }

        format!("{{{}\n}}", variants)
    }

    fn gen_tuple(&self, t: &TTuple) -> String {
        let mut values = String::new();

        for (n, item) in t.items.iter().enumerate() {
            let is_last = n == t.items.len() - 1;

            let value = match item {
                TupleItem::Reference(d) => d.name,
                TupleItem::TPrimitive(p) => self.gen_primitive_type(p),
            };

            values.push_str(value);
            if !is_last {
                values.push_str(", ");
            }
        }

        format!("({})", values)
    }

    fn gen_primitive_type(&self, ty: &TPrimitive) -> &'static str {
        match ty {
            TPrimitive::String => "String",
            TPrimitive::Tbool => "bool",
            TPrimitive::Ti64 => "i64",
            TPrimitive::Tf64 => "f64",
        }
    }
}
