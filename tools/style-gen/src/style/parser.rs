//! Reads `.style` modules and evaluates their values, following
//! `codegen/style/parsed_file.cpp`: a value is fully resolved when it is read,
//! a struct value starts from its type's defaults, parents fill the fields that
//! the child has not set, and a name refers to a variable of this module or of
//! a module it includes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{Context as _, anyhow, bail, ensure};

use super::lexer::{Located, Token, tokenize};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Type {
    Int,
    Bool,
    Double,
    Pixels,
    Str,
    Color,
    Point,
    Size,
    Align,
    Margins,
    Font,
    Icon,
    Struct(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Color {
    /// A palette entry; themes can change it.
    Palette(String),
    Rgba([u8; 4]),
}

/// `transparent` in a style file.
const TRANSPARENT: [u8; 4] = [255, 255, 255, 0];
/// The value of a color field nobody set.
const UNSET_COLOR: [u8; 4] = [0, 0, 0, 255];
/// The size of a font field nobody set.
const UNSET_FONT_SIZE: i32 = 13;

pub(crate) const FONT_BOLD: u8 = 0x01;
pub(crate) const FONT_ITALIC: u8 = 0x02;
pub(crate) const FONT_UNDERLINE: u8 = 0x04;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Font {
    pub(crate) family: String,
    pub(crate) size: i32,
    pub(crate) flags: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum IconMask {
    /// An image in an `icons` directory, `name-modifier-modifier` in the source.
    File {
        name: String,
        modifiers: Vec<String>,
        path: PathBuf,
    },
    /// `size(w, h)`: a filled rectangle.
    Rect { width: i32, height: i32 },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MonoIcon {
    pub(crate) mask: IconMask,
    pub(crate) color: Color,
    pub(crate) padding: [i32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Unset,
    Inherited,
    Explicit,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Field {
    pub(crate) name: String,
    pub(crate) value: Value,
    status: Status,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    Int(i32),
    Bool(bool),
    Double(f64),
    Pixels(i32),
    Str(String),
    Color(Color),
    Point(i32, i32),
    Size(i32, i32),
    Align(String),
    Margins([i32; 4]),
    Font(Font),
    Icon(Vec<MonoIcon>),
    Struct { ty: String, fields: Vec<Field> },
}

impl Value {
    pub(crate) fn ty(&self) -> Type {
        match self {
            Self::Int(_) => Type::Int,
            Self::Bool(_) => Type::Bool,
            Self::Double(_) => Type::Double,
            Self::Pixels(_) => Type::Pixels,
            Self::Str(_) => Type::Str,
            Self::Color(_) => Type::Color,
            Self::Point(..) => Type::Point,
            Self::Size(..) => Type::Size,
            Self::Align(_) => Type::Align,
            Self::Margins(_) => Type::Margins,
            Self::Font(_) => Type::Font,
            Self::Icon(_) => Type::Icon,
            Self::Struct { ty, .. } => Type::Struct(ty.clone()),
        }
    }

    fn is_numeric(&self) -> bool {
        matches!(self, Self::Int(_) | Self::Double(_) | Self::Pixels(_))
    }
}

#[derive(Debug)]
pub(crate) struct StructDef {
    pub(crate) name: String,
    pub(crate) fields: Vec<(String, Type)>,
}

#[derive(Debug)]
pub(crate) struct Variable {
    pub(crate) name: String,
    pub(crate) value: Value,
}

#[derive(Debug, Default)]
pub(crate) struct Module {
    /// The name `using` refers to it by, such as `ui/basic.style`.
    pub(crate) name: String,
    pub(crate) includes: Vec<Rc<Module>>,
    pub(crate) structs: Vec<StructDef>,
    pub(crate) variables: Vec<Variable>,
    variable_index: HashMap<String, usize>,
    struct_index: HashMap<String, usize>,
}

impl Module {
    fn find_variable(&self, name: &str) -> Option<&Value> {
        if let Some(&index) = self.variable_index.get(name) {
            return Some(&self.variables[index].value);
        }
        self.includes
            .iter()
            .find_map(|module| module.find_variable(name))
    }

    fn find_struct(&self, name: &str) -> Option<&StructDef> {
        if let Some(&index) = self.struct_index.get(name) {
            return Some(&self.structs[index]);
        }
        self.includes
            .iter()
            .find_map(|module| module.find_struct(name))
    }

    /// Every module this one includes, directly or not, dependencies first.
    pub(crate) fn dependencies(self: &Rc<Self>) -> Vec<Rc<Self>> {
        let mut out: Vec<Rc<Self>> = Vec::new();
        fn visit(module: &Rc<Module>, out: &mut Vec<Rc<Module>>) {
            for included in &module.includes {
                visit(included, out);
            }
            if !out.iter().any(|known| Rc::ptr_eq(known, module)) {
                out.push(Rc::clone(module));
            }
        }
        visit(self, &mut out);
        out
    }
}

/// Finds and caches modules by their `using` name.
#[derive(Debug)]
pub(crate) struct Loader {
    include_dirs: Vec<PathBuf>,
    icon_dirs: Vec<PathBuf>,
    palette_names: Vec<String>,
    cache: HashMap<String, Rc<Module>>,
}

impl Loader {
    pub(crate) fn new(
        include_dirs: Vec<PathBuf>,
        icon_dirs: Vec<PathBuf>,
        palette_names: Vec<String>,
    ) -> Self {
        Self {
            include_dirs,
            icon_dirs,
            palette_names,
            cache: HashMap::new(),
        }
    }

    pub(crate) fn load(&mut self, name: &str) -> anyhow::Result<Rc<Module>> {
        self.load_from(name, &mut Vec::new())
    }

    fn load_from(&mut self, name: &str, stack: &mut Vec<String>) -> anyhow::Result<Rc<Module>> {
        if let Some(module) = self.cache.get(name) {
            return Ok(Rc::clone(module));
        }
        ensure!(
            !stack.iter().any(|open| open == name),
            "include cycle through {name}"
        );
        let module = if name.ends_with(".palette") {
            self.palette_module(name)
        } else {
            let path = self.find_file(name)?;
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            stack.push(name.to_owned());
            let module = Parser::new(self, name, tokenize(&source)?, stack)
                .read()
                .with_context(|| format!("in {name}"))?;
            stack.pop();
            module
        };
        let module = Rc::new(module);
        self.cache.insert(name.to_owned(), Rc::clone(&module));
        Ok(module)
    }

    /// The palette's colors are references, so a theme can replace them.
    fn palette_module(&self, name: &str) -> Module {
        let mut module = Module {
            name: name.to_owned(),
            ..Module::default()
        };
        for color in &self.palette_names {
            module
                .variable_index
                .insert(color.clone(), module.variables.len());
            module.variables.push(Variable {
                name: color.clone(),
                value: Value::Color(Color::Palette(color.clone())),
            });
        }
        module
    }

    fn find_file(&self, name: &str) -> anyhow::Result<PathBuf> {
        self.include_dirs
            .iter()
            .map(|dir| dir.join(name))
            .find(|path| path.is_file())
            .ok_or_else(|| anyhow!("{name} not found in the include directories"))
    }

    fn find_icon(&self, name: &str) -> Option<PathBuf> {
        self.icon_dirs.iter().find_map(|dir| {
            ["png", "svg"]
                .iter()
                .map(|extension| dir.join(format!("{name}.{extension}")))
                .find(|path| path.is_file())
        })
    }
}

const ICON_MODIFIERS: &[&str] = &[
    "invert",
    "flip_horizontal",
    "flip_vertical",
    "rotate_cw",
    "rotate_ccw",
];

/// Tries to read one kind of value; `None` leaves the position for the next.
type Reader<'a> = fn(&mut Parser<'a>) -> anyhow::Result<Option<Value>>;

struct Parser<'a> {
    loader: &'a mut Loader,
    stack: &'a mut Vec<String>,
    tokens: Vec<Located>,
    position: usize,
    module: Module,
}

impl<'a> Parser<'a> {
    fn new(
        loader: &'a mut Loader,
        name: &str,
        tokens: Vec<Located>,
        stack: &'a mut Vec<String>,
    ) -> Self {
        Self {
            loader,
            stack,
            tokens,
            position: 0,
            module: Module {
                name: name.to_owned(),
                ..Module::default()
            },
        }
    }

    fn read(mut self) -> anyhow::Result<Module> {
        while self.position < self.tokens.len() {
            let line = self.line();
            self.read_definition()
                .with_context(|| format!("line {line}"))?;
        }
        Ok(self.module)
    }

    fn read_definition(&mut self) -> anyhow::Result<()> {
        let name = self.expect_name()?;
        if name == "using" {
            let Some(Token::Str(included)) = self.next() else {
                bail!("expected a file name after `using`");
            };
            self.expect(';')?;
            let module = self.loader.load_from(&included, self.stack)?;
            self.module.includes.push(module);
        } else if self.accept('{') {
            let definition = self.read_struct_definition(&name)?;
            ensure!(
                self.module.find_struct(&name).is_none(),
                "struct {name} is already defined"
            );
            self.module
                .struct_index
                .insert(name, self.module.structs.len());
            self.module.structs.push(definition);
        } else if self.accept(':') {
            let value = self.read_variable_value()?;
            ensure!(
                !self.module.variable_index.contains_key(&name),
                "{name} is already defined"
            );
            self.module
                .variable_index
                .insert(name.clone(), self.module.variables.len());
            self.module.variables.push(Variable { name, value });
        } else {
            bail!("expected `{{` or `:` after {name}");
        }
        Ok(())
    }

    fn read_struct_definition(&mut self, name: &str) -> anyhow::Result<StructDef> {
        let mut fields = Vec::new();
        while !self.accept('}') {
            let field = self.expect_name()?;
            self.expect(':')?;
            let type_name = self.expect_name()?;
            let ty = match type_name.as_str() {
                "int" => Type::Int,
                "bool" => Type::Bool,
                "double" => Type::Double,
                "pixels" => Type::Pixels,
                "string" => Type::Str,
                "color" => Type::Color,
                "point" => Type::Point,
                "size" => Type::Size,
                "align" => Type::Align,
                "margins" => Type::Margins,
                "font" => Type::Font,
                "icon" => Type::Icon,
                other => {
                    ensure!(
                        self.module.find_struct(other).is_some(),
                        "unknown type {other}"
                    );
                    Type::Struct(other.to_owned())
                }
            };
            self.expect(';')?;
            fields.push((field, ty));
        }
        ensure!(!fields.is_empty(), "struct {name} has no fields");
        Ok(StructDef {
            name: name.to_owned(),
            fields,
        })
    }

    /// A value followed by `;`; struct literals end with `}` instead.
    fn read_variable_value(&mut self) -> anyhow::Result<Value> {
        let value = self.read_value()?;
        let ended_with_brace = self.previous() == Some(&Token::Punct('}'));
        if !(matches!(value, Value::Struct { .. }) && ended_with_brace) {
            self.expect(';')?;
        }
        Ok(value)
    }

    fn read_value(&mut self) -> anyhow::Result<Value> {
        let start = self.position;
        let readers: [Reader<'a>; 12] = [
            Self::read_color,
            Self::read_point,
            Self::read_size,
            Self::read_bool,
            Self::read_align,
            Self::read_margins,
            Self::read_font,
            Self::read_icon,
            Self::read_numeric,
            Self::read_string,
            Self::read_struct,
            Self::read_copy,
        ];
        for reader in readers {
            if let Some(value) = reader(self)? {
                return Ok(value);
            }
            self.position = start;
        }
        bail!("expected a value, found {:?}", self.peek())
    }

    fn read_color(&mut self) -> anyhow::Result<Option<Value>> {
        match self.peek() {
            Some(Token::Punct('#')) => bail!("literal colors belong in the palette"),
            Some(Token::Name(name)) if name == "transparent" => {
                self.position += 1;
                Ok(Some(Value::Color(Color::Rgba(TRANSPARENT))))
            }
            _ => Ok(None),
        }
    }

    fn read_point(&mut self) -> anyhow::Result<Option<Value>> {
        let Some([x, y]) = self.read_pixel_tuple("point")? else {
            return Ok(None);
        };
        Ok(Some(Value::Point(x, y)))
    }

    fn read_size(&mut self) -> anyhow::Result<Option<Value>> {
        let Some([width, height]) = self.read_pixel_tuple("size")? else {
            return Ok(None);
        };
        Ok(Some(Value::Size(width, height)))
    }

    fn read_margins(&mut self) -> anyhow::Result<Option<Value>> {
        Ok(self.read_pixel_tuple("margins")?.map(Value::Margins))
    }

    /// `keyword(px, px, ...)` with exactly `N` pixel values.
    fn read_pixel_tuple<const N: usize>(
        &mut self,
        keyword: &str,
    ) -> anyhow::Result<Option<[i32; N]>> {
        if !self.accept_name(keyword) {
            return Ok(None);
        }
        self.expect('(')?;
        let mut values = [0; N];
        for (index, slot) in values.iter_mut().enumerate() {
            if index > 0 {
                self.expect(',')?;
            }
            match self.read_numeric_or_copy_required()? {
                Value::Pixels(value) => *slot = value,
                other => bail!("{keyword} takes px values, found {other:?}"),
            }
        }
        self.expect(')')?;
        Ok(Some(values))
    }

    fn read_bool(&mut self) -> anyhow::Result<Option<Value>> {
        if self.accept_name("true") {
            Ok(Some(Value::Bool(true)))
        } else if self.accept_name("false") {
            Ok(Some(Value::Bool(false)))
        } else {
            Ok(None)
        }
    }

    fn read_align(&mut self) -> anyhow::Result<Option<Value>> {
        if !self.accept_name("align") {
            return Ok(None);
        }
        self.expect('(')?;
        let align = self.expect_name()?;
        self.expect(')')?;
        ensure!(
            align.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "bad align {align}"
        );
        Ok(Some(Value::Align(align)))
    }

    fn read_font(&mut self) -> anyhow::Result<Option<Value>> {
        if !self.accept_name("font") {
            return Ok(None);
        }
        self.expect('(')?;
        let mut font = Font {
            family: String::new(),
            size: 0,
            flags: 0,
        };
        let mut size = None;
        loop {
            if self.accept_name("bold") || self.accept_name("semibold") {
                font.flags |= FONT_BOLD;
            } else if self.accept_name("italic") {
                font.flags |= FONT_ITALIC;
            } else if self.accept_name("underline") {
                font.flags |= FONT_UNDERLINE;
            } else if let Some(family) = self.read_string_or_copy()? {
                font.family = family;
            } else if let Some(value) = self.read_numeric_or_copy()? {
                size = Some(value);
            } else if self.accept(')') {
                break;
            } else {
                bail!("expected a font family, size or `)`");
            }
        }
        match size {
            Some(Value::Pixels(px)) => font.size = px,
            other => bail!("a font needs a px size, found {other:?}"),
        }
        Ok(Some(Value::Font(font)))
    }

    fn read_icon(&mut self) -> anyhow::Result<Option<Value>> {
        if !self.accept_name("icon") {
            return Ok(None);
        }
        let mut parts = Vec::new();
        if self.accept('{') {
            while !self.accept('}') {
                self.expect('{')?;
                parts.push(self.read_mono_icon()?);
                self.expect('}')?;
                self.accept(',');
            }
        } else if self.accept('(') {
            parts.push(self.read_mono_icon()?);
            self.expect(')')?;
        }
        Ok(Some(Value::Icon(parts)))
    }

    fn read_mono_icon(&mut self) -> anyhow::Result<MonoIcon> {
        let mask = match self.read_value()? {
            Value::Str(source) => self.icon_file(&source)?,
            Value::Size(width, height) => IconMask::Rect { width, height },
            other => bail!("expected an icon file or size, found {other:?}"),
        };
        let mut icon = MonoIcon {
            mask,
            color: Color::Rgba(UNSET_COLOR),
            padding: [0; 4],
        };
        if self.accept(',') {
            match self.read_value()? {
                Value::Color(color) => icon.color = color,
                other => bail!("expected an icon color, found {other:?}"),
            }
            if self.accept(',') {
                icon.padding = match self.read_value()? {
                    Value::Margins(margins) => margins,
                    Value::Point(x, y) => [x, y, 0, 0],
                    other => bail!("expected an icon offset, found {other:?}"),
                };
            }
        }
        Ok(icon)
    }

    fn icon_file(&self, source: &str) -> anyhow::Result<IconMask> {
        let mut pieces = source.split('-');
        // tdesktop writes some masks from the icons root, as in "/menu/article_rich".
        let name = pieces
            .next()
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_owned();
        let modifiers: Vec<String> = pieces.map(str::to_owned).collect();
        for modifier in &modifiers {
            let is_size = modifier
                .split_once('x')
                .is_some_and(|(w, h)| w.parse::<u32>().is_ok() && h.parse::<u32>().is_ok());
            ensure!(
                is_size || ICON_MODIFIERS.contains(&modifier.as_str()),
                "unknown icon modifier {modifier} in {source}"
            );
        }
        let path = self
            .loader
            .find_icon(&name)
            .ok_or_else(|| anyhow!("icon {name} not found"))?;
        Ok(IconMask::File {
            name,
            modifiers,
            path,
        })
    }

    fn read_positive(&mut self) -> Option<Value> {
        let value = match self.peek()? {
            Token::Int(value) => Value::Int(*value),
            Token::Double(value) => Value::Double(*value),
            Token::Name(name) => {
                let digits = name.strip_suffix("px")?;
                if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                Value::Pixels(digits.parse().ok()?)
            }
            _ => return None,
        };
        self.position += 1;
        Some(value)
    }

    /// A number, or a copy of a numeric variable.
    fn read_numeric_or_copy(&mut self) -> anyhow::Result<Option<Value>> {
        if let Some(value) = self.read_numeric()? {
            return Ok(Some(value));
        }
        let start = self.position;
        match self.read_copy()? {
            Some(value) if value.is_numeric() => Ok(Some(value)),
            _ => {
                self.position = start;
                Ok(None)
            }
        }
    }

    fn read_numeric_or_copy_required(&mut self) -> anyhow::Result<Value> {
        self.read_numeric_or_copy()?
            .ok_or_else(|| anyhow!("expected a number, found {:?}", self.peek()))
    }

    /// `a + b - c`, with `-` allowed in front.
    fn read_numeric(&mut self) -> anyhow::Result<Option<Value>> {
        if self.accept('-') {
            let Some(value) = self.read_numeric()? else {
                bail!("expected a number after `-`");
            };
            return Ok(Some(match value {
                Value::Int(v) => Value::Int(-v),
                Value::Pixels(v) => Value::Pixels(-v),
                Value::Double(v) => Value::Double(-v),
                other => other,
            }));
        }
        let mut value = match self.read_positive() {
            Some(value) => value,
            None => {
                let start = self.position;
                match self.read_copy()? {
                    Some(copy) if copy.is_numeric() => copy,
                    _ => {
                        self.position = start;
                        return Ok(None);
                    }
                }
            }
        };
        loop {
            let sign = if self.accept('+') {
                1
            } else if self.accept('-') {
                -1
            } else {
                break;
            };
            let right = self.read_numeric_or_copy_required()?;
            value = match (value, right) {
                (Value::Pixels(a), Value::Pixels(b)) => Value::Pixels(a + sign * b),
                (Value::Int(a), Value::Int(b)) => Value::Int(a + sign * b),
                (Value::Double(a), Value::Double(b)) => Value::Double(a + f64::from(sign) * b),
                (a, b) => bail!("cannot combine {a:?} and {b:?}"),
            };
        }
        Ok(Some(value))
    }

    fn read_string(&mut self) -> anyhow::Result<Option<Value>> {
        match self.peek() {
            Some(Token::Str(value)) => {
                ensure!(value.is_ascii(), "style strings must be ASCII: {value:?}");
                let value = value.clone();
                self.position += 1;
                Ok(Some(Value::Str(value)))
            }
            _ => Ok(None),
        }
    }

    fn read_string_or_copy(&mut self) -> anyhow::Result<Option<String>> {
        if let Some(Value::Str(value)) = self.read_string()? {
            return Ok(Some(value));
        }
        let start = self.position;
        match self.read_copy()? {
            Some(Value::Str(value)) => Ok(Some(value)),
            _ => {
                self.position = start;
                Ok(None)
            }
        }
    }

    fn read_struct(&mut self) -> anyhow::Result<Option<Value>> {
        let Some(Token::Name(name)) = self.peek() else {
            return Ok(None);
        };
        let name = name.clone();
        let Some(mut value) = self.default_struct(&name) else {
            return Ok(None);
        };
        self.position += 1;
        if self.accept('(') {
            loop {
                let parent = self.expect_name()?;
                self.apply_parent(&mut value, &parent)?;
                if self.accept(')') {
                    break;
                }
                self.expect(',')?;
            }
        }
        self.expect('{')?;
        while !self.accept('}') {
            let field = self.expect_name()?;
            self.expect(':')?;
            let field_value = self.read_variable_value()?;
            assign_field(&mut value, &field, field_value)?;
        }
        Ok(Some(value))
    }

    fn default_struct(&self, name: &str) -> Option<Value> {
        let definition = self.module.find_struct(name)?;
        let fields = definition
            .fields
            .iter()
            .map(|(field, ty)| Field {
                name: field.clone(),
                value: self.default_value(ty),
                status: Status::Unset,
            })
            .collect();
        Some(Value::Struct {
            ty: name.to_owned(),
            fields,
        })
    }

    /// What `Value(type, Qt::Uninitialized)` holds.
    fn default_value(&self, ty: &Type) -> Value {
        match ty {
            Type::Int => Value::Int(0),
            Type::Bool => Value::Bool(false),
            Type::Double => Value::Double(0.0),
            Type::Pixels => Value::Pixels(0),
            Type::Str => Value::Str(String::new()),
            Type::Color => Value::Color(Color::Rgba(UNSET_COLOR)),
            Type::Point => Value::Point(0, 0),
            Type::Size => Value::Size(0, 0),
            Type::Align => Value::Align("topleft".to_owned()),
            Type::Margins => Value::Margins([0; 4]),
            Type::Font => Value::Font(Font {
                family: String::new(),
                size: UNSET_FONT_SIZE,
                flags: 0,
            }),
            Type::Icon => Value::Icon(Vec::new()),
            Type::Struct(name) => self.default_struct(name).unwrap_or_else(|| Value::Struct {
                ty: name.clone(),
                fields: Vec::new(),
            }),
        }
    }

    /// Copies the parent's fields that it set explicitly, or that the child has
    /// not set at all (`applyStructParent`).
    fn apply_parent(&self, value: &mut Value, parent_name: &str) -> anyhow::Result<()> {
        let parent = self
            .module
            .find_variable(parent_name)
            .ok_or_else(|| anyhow!("parent {parent_name} not found"))?;
        let (
            Value::Struct {
                ty: parent_ty,
                fields: parent_fields,
            },
            Value::Struct { ty, fields },
        ) = (parent, value)
        else {
            bail!("parent {parent_name} is not a struct");
        };
        ensure!(
            parent_ty == ty,
            "parent {parent_name} is a {parent_ty}, not a {ty}"
        );
        for (source, target) in parent_fields.iter().zip(fields.iter_mut()) {
            if source.status == Status::Explicit || target.status == Status::Unset {
                target.value = source.value.clone();
                target.status = Status::Inherited;
            }
        }
        Ok(())
    }

    /// A variable's value, with `.field` access into structs, sizes, points and
    /// margins (`readCopyValue`).
    fn read_copy(&mut self) -> anyhow::Result<Option<Value>> {
        let Some(Token::Name(name)) = self.peek() else {
            return Ok(None);
        };
        let name = name.clone();
        let Some(mut value) = self.module.find_variable(&name).cloned() else {
            return Ok(None);
        };
        self.position += 1;
        while self.accept('.') {
            let field = self.expect_name()?;
            value = match (&value, field.as_str()) {
                (Value::Size(w, _), "width") => return Ok(Some(Value::Pixels(*w))),
                (Value::Size(_, h), "height") => return Ok(Some(Value::Pixels(*h))),
                (Value::Point(x, _), "x") => return Ok(Some(Value::Pixels(*x))),
                (Value::Point(_, y), "y") => return Ok(Some(Value::Pixels(*y))),
                (Value::Margins(m), side) => {
                    let px = match side {
                        "left" => m[0],
                        "top" => m[1],
                        "right" => m[2],
                        "bottom" => m[3],
                        "leftRight" => m[0] + m[2],
                        "topBottom" => m[1] + m[3],
                        _ => bail!("margins have no field {side}"),
                    };
                    return Ok(Some(Value::Pixels(px)));
                }
                (Value::Struct { fields, .. }, _) => fields
                    .iter()
                    .find(|f| f.name == field)
                    .map(|f| f.value.clone())
                    .ok_or_else(|| anyhow!("{name} has no field {field}"))?,
                _ => bail!("{name} is not a struct"),
            };
        }
        Ok(Some(value))
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position).map(|t| &t.token)
    }

    fn previous(&self) -> Option<&Token> {
        self.position
            .checked_sub(1)
            .and_then(|i| self.tokens.get(i))
            .map(|t| &t.token)
    }

    fn line(&self) -> usize {
        self.tokens.get(self.position).map_or(0, |t| t.line)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).map(|t| t.token.clone());
        self.position += 1;
        token
    }

    fn accept(&mut self, punct: char) -> bool {
        if self.peek() == Some(&Token::Punct(punct)) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn accept_name(&mut self, keyword: &str) -> bool {
        if matches!(self.peek(), Some(Token::Name(name)) if name == keyword) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, punct: char) -> anyhow::Result<()> {
        ensure!(
            self.accept(punct),
            "expected `{punct}`, found {:?}",
            self.peek()
        );
        Ok(())
    }

    fn expect_name(&mut self) -> anyhow::Result<String> {
        match self.next() {
            Some(Token::Name(name)) => Ok(name),
            other => bail!("expected a name, found {other:?}"),
        }
    }
}

fn assign_field(value: &mut Value, name: &str, field_value: Value) -> anyhow::Result<()> {
    let Value::Struct { ty, fields } = value else {
        bail!("not a struct");
    };
    let field = fields
        .iter_mut()
        .find(|f| f.name == name)
        .ok_or_else(|| anyhow!("{ty} has no field {name}"))?;
    ensure!(
        field.value.ty() == field_value.ty(),
        "{ty}.{name} is a {:?}, not a {:?}",
        field.value.ty(),
        field_value.ty()
    );
    field.value = field_value;
    field.status = Status::Explicit;
    Ok(())
}

/// The tdesktop directories `.style` files and icons are found in.
pub(crate) fn tdesktop_dirs(tdesktop: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let telegram = tdesktop.join("Telegram");
    let include_dirs = vec![telegram.join("lib_ui"), telegram.join("SourceFiles")];
    let icon_dirs = vec![
        telegram.join("lib_ui/icons"),
        telegram.join("Resources/icons"),
    ];
    (include_dirs, icon_dirs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(source: &str) -> Module {
        let mut loader = Loader::new(Vec::new(), Vec::new(), vec!["windowFg".into()]);
        let palette = loader.load("colors.palette").unwrap();
        let mut stack = Vec::new();
        let mut parser = Parser::new(
            &mut loader,
            "test.style",
            tokenize(source).unwrap(),
            &mut stack,
        );
        parser.module.includes.push(palette);
        parser.read().unwrap()
    }

    fn field<'a>(module: &'a Module, variable: &str, field: &str) -> &'a Value {
        let Some(Value::Struct { fields, .. }) = module.find_variable(variable) else {
            panic!("{variable} is not a struct");
        };
        &fields.iter().find(|f| f.name == field).unwrap().value
    }

    const SOURCE: &str = "
Label { width: pixels; fg: color; scale: double; inner: Inner; }
Inner { size: pixels; }
";

    #[test]
    fn parents_fill_only_unset_fields() {
        let module = module(
            "Inner { size: pixels; }
Label { width: pixels; fg: color; inner: Inner; }
base: 10px;
parent: Label { width: base + 2px; fg: windowFg; inner: Inner { size: 3px; } }
child: Label(parent) { width: -4px; }
copy: child;
",
        );
        assert_eq!(field(&module, "child", "width"), &Value::Pixels(-4));
        assert_eq!(
            field(&module, "child", "fg"),
            &Value::Color(Color::Palette("windowFg".into()))
        );
        assert_eq!(field(&module, "parent", "width"), &Value::Pixels(12));
        assert_eq!(field(&module, "copy", "width"), &Value::Pixels(-4));
    }

    #[test]
    fn rejects_type_mismatches() {
        let mut loader = Loader::new(Vec::new(), Vec::new(), Vec::new());
        let mut stack = Vec::new();
        let tokens = tokenize(&format!("{SOURCE}x: Label {{ width: 1; }}")).unwrap();
        let error = Parser::new(&mut loader, "t.style", tokens, &mut stack)
            .read()
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("unknown type Inner"),
            "{error:#}"
        );
    }
}
