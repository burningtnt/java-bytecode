//! Parser and printer for generic signatures (JVMS §4.7.9.1) and descriptors.
//!
//! This mirrors `jdk.internal.classfile.SignaturesImpl` and
//! `com.sun.tools.javap.ClassWriter.SignaturePrinter` so that generic
//! declarations render exactly like javap's.

use java_bytecode::FieldType;

/// A parsed type signature.
#[derive(Debug, Clone)]
pub enum Sig {
    /// A base type (`B C D F I J S Z V`).
    Base(char),
    /// A class or interface type, possibly nested and parameterized.
    Class {
        outer: Option<Box<Sig>>,
        name: String,
        args: Vec<TypeArg>,
    },
    /// A type variable (`Tname;`).
    Var(String),
    /// An array type.
    Array(Box<Sig>),
}

/// A type argument.
#[derive(Debug, Clone)]
pub enum TypeArg {
    /// `*`
    Unbounded,
    /// `+Type`
    Extends(Sig),
    /// `-Type`
    Super(Sig),
    /// `Type`
    Exact(Sig),
}

/// A type parameter declaration.
#[derive(Debug, Clone)]
pub struct TypeParam {
    pub id: String,
    pub class_bound: Option<Sig>,
    pub interface_bounds: Vec<Sig>,
}

/// A class signature.
#[derive(Debug, Clone)]
pub struct ClassSig {
    pub params: Vec<TypeParam>,
    pub superclass: Option<Sig>,
    pub interfaces: Vec<Sig>,
}

/// A method signature.
#[derive(Debug, Clone)]
pub struct MethodSig {
    pub params: Vec<TypeParam>,
    pub args: Vec<Sig>,
    pub result: Sig,
    pub throws: Vec<Sig>,
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

fn is_non_identifier(c: char) -> bool {
    matches!(c, '.' | ';' | '/' | '<' | '>' | ':' | '[')
}

impl Parser {
    fn new(s: &str) -> Self {
        Self {
            chars: s.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn matches(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn require(&mut self, c: char) {
        assert!(self.matches(c), "expected {c:?} in signature");
    }

    fn identifier(&mut self) -> String {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if is_non_identifier(c) {
                break;
            }
            self.pos += 1;
        }
        assert!(self.pos > start, "expected an identifier in signature");
        self.chars[start..self.pos].iter().collect()
    }

    fn parse_class_signature(&mut self) -> ClassSig {
        let params = self.parse_param_types();
        let superclass = Some(self.class_type_sig());
        let mut interfaces = Vec::new();
        while self.pos < self.chars.len() {
            interfaces.push(self.class_type_sig());
        }
        ClassSig {
            params,
            superclass,
            interfaces,
        }
    }

    fn parse_method_signature(&mut self) -> MethodSig {
        let params = self.parse_param_types();
        self.require('(');
        let mut args = Vec::new();
        while !self.matches(')') {
            args.push(self.type_sig());
        }
        let result = self.type_sig();
        let mut throws = Vec::new();
        while self.pos < self.chars.len() {
            self.require('^');
            throws.push(self.reference_type_sig());
        }
        MethodSig {
            params,
            args,
            result,
            throws,
        }
    }

    fn parse_type_signature(&mut self) -> Sig {
        let s = self.type_sig();
        assert!(
            self.pos == self.chars.len(),
            "trailing characters in type signature"
        );
        s
    }

    fn parse_param_types(&mut self) -> Vec<TypeParam> {
        let mut params = Vec::new();
        if self.matches('<') {
            loop {
                let id = self.identifier();
                let mut class_bound = None;
                let mut interface_bounds = Vec::new();
                self.require(':');
                if self.peek() != Some(':') {
                    // The class bound is absent when the next token is `>` or `:`.
                    let end = self.next_identifier_end();
                    if let Some(limit) = self.chars.get(end).copied() {
                        if limit != '>' && limit != ':' {
                            class_bound = Some(self.reference_type_sig());
                        }
                    }
                }
                while self.matches(':') {
                    interface_bounds.push(self.reference_type_sig());
                }
                params.push(TypeParam {
                    id,
                    class_bound,
                    interface_bounds,
                });
                if self.matches('>') {
                    break;
                }
            }
        }
        params
    }

    fn next_identifier_end(&self) -> usize {
        let mut end = self.pos;
        while let Some(c) = self.chars.get(end) {
            if is_non_identifier(*c) {
                break;
            }
            end += 1;
        }
        end
    }

    fn type_sig(&mut self) -> Sig {
        let c = self.peek().expect("type signature");
        match c {
            'B' | 'C' | 'D' | 'F' | 'I' | 'J' | 'V' | 'S' | 'Z' => {
                self.pos += 1;
                Sig::Base(c)
            }
            _ => self.reference_type_sig(),
        }
    }

    fn reference_type_sig(&mut self) -> Sig {
        match self.peek() {
            Some('L') => self.class_type_sig(),
            Some('T') => {
                self.pos += 1;
                let id = self.identifier();
                self.require(';');
                Sig::Var(id)
            }
            Some('[') => {
                self.pos += 1;
                Sig::Array(Box::new(self.type_sig()))
            }
            other => panic!("unexpected {other:?} in reference type signature"),
        }
    }

    fn type_arg(&mut self) -> TypeArg {
        match self.peek() {
            Some('*') => {
                self.pos += 1;
                TypeArg::Unbounded
            }
            Some('+') => {
                self.pos += 1;
                TypeArg::Extends(self.reference_type_sig())
            }
            Some('-') => {
                self.pos += 1;
                TypeArg::Super(self.reference_type_sig())
            }
            _ => TypeArg::Exact(self.reference_type_sig()),
        }
    }

    fn class_type_sig(&mut self) -> Sig {
        self.require('L');
        let mut current: Option<Sig> = None;
        loop {
            let start = self.pos;
            self.identifier();
            if current.is_none() {
                while self.matches('/') {
                    self.identifier();
                }
            }
            let name: String = self.chars[start..self.pos].iter().collect();

            let mut args = Vec::new();
            if self.matches('<') {
                loop {
                    args.push(self.type_arg());
                    if self.matches('>') {
                        break;
                    }
                }
            }

            let end = self.matches(';');
            let nested = self.matches('.');
            let sig = Sig::Class {
                outer: current.map(Box::new),
                name,
                args,
            };
            if end {
                return sig;
            }
            assert!(nested, "expected '.' or ';' in class type signature");
            current = Some(sig);
        }
    }
}

/// Parses a class signature (`Signature` attribute of a class).
pub fn parse_class_signature(s: &str) -> ClassSig {
    Parser::new(s).parse_class_signature()
}

/// Parses a method signature (`Signature` attribute of a method).
pub fn parse_method_signature(s: &str) -> MethodSig {
    Parser::new(s).parse_method_signature()
}

/// Parses a single type signature (used for field signatures and annotation
/// class descriptors).
pub fn parse_type_signature(s: &str) -> Sig {
    Parser::new(s).parse_type_signature()
}

/// Converts a parsed descriptor field type into a signature.
pub fn sig_from_field_type(ty: &FieldType<'_>) -> Sig {
    match ty {
        FieldType::Base(base) => Sig::Base(char::from(base.as_byte())),
        FieldType::Object(name) => Sig::Class {
            outer: None,
            name: name.to_string_lossy(),
            args: Vec::new(),
        },
        FieldType::Array(component) => Sig::Array(Box::new(sig_from_field_type(component))),
    }
}

/// Renders signatures the way `javap`'s `SignaturePrinter` does.
pub struct SigPrinter {
    pub verbose: bool,
}

impl SigPrinter {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }

    fn java_name(name: &str) -> String {
        name.replace('/', ".")
    }

    pub fn print_class(&self, cs: &ClassSig, is_interface: bool) -> String {
        let mut sb = String::new();
        sb.push_str(&self.print_type_params(&cs.params));
        if is_interface {
            let mut sep = " extends ";
            for sig in &cs.interfaces {
                sb.push_str(sep);
                sb.push_str(&self.print(sig));
                sep = ", ";
            }
        } else {
            if let Some(sig) = &cs.superclass {
                if self.verbose || !is_object(sig) {
                    sb.push_str(" extends ");
                    sb.push_str(&self.print(sig));
                }
            }
            let mut sep = " implements ";
            for sig in &cs.interfaces {
                sb.push_str(sep);
                sb.push_str(&self.print(sig));
                sep = ", ";
            }
        }
        sb
    }

    pub fn print_type_params(&self, tps: &[TypeParam]) -> String {
        if tps.is_empty() {
            return String::new();
        }
        let mut sb = String::from("<");
        let mut sep = "";
        for tp in tps {
            sb.push_str(sep);
            sb.push_str(&tp.id);
            sep = " extends ";
            if let Some(bound) = &tp.class_bound {
                if self.verbose || !is_object(bound) {
                    sb.push_str(sep);
                    sb.push_str(&self.print(bound));
                    sep = " & ";
                }
            }
            for bound in &tp.interface_bounds {
                sb.push_str(sep);
                sb.push_str(&self.print(bound));
                sep = " & ";
            }
            sep = ", ";
        }
        sb.push('>');
        sb
    }

    pub fn print_list(&self, prefix: &str, args: &[Sig], postfix: &str) -> String {
        let mut sb = String::from(prefix);
        let mut sep = "";
        for arg in args {
            sb.push_str(sep);
            sb.push_str(&self.print(arg));
            sep = ", ";
        }
        sb.push_str(postfix);
        sb
    }

    pub fn print(&self, sig: &Sig) -> String {
        let mut sb = String::new();
        self.print_into(&mut sb, sig);
        sb
    }

    fn print_into(&self, sb: &mut String, sig: &Sig) {
        match sig {
            Sig::Base(c) => sb.push_str(base_display_name(*c)),
            Sig::Class { outer, name, args } => {
                if let Some(outer) = outer {
                    self.print_into(sb, outer);
                    sb.push('.');
                }
                sb.push_str(&Self::java_name(name));
                if !args.is_empty() {
                    let mut sep = "";
                    sb.push('<');
                    for arg in args {
                        sb.push_str(sep);
                        self.print_type_arg(sb, arg);
                        sep = ", ";
                    }
                    sb.push('>');
                }
            }
            Sig::Var(id) => sb.push_str(id),
            Sig::Array(component) => {
                self.print_into(sb, component);
                sb.push_str("[]");
            }
        }
    }

    fn print_type_arg(&self, sb: &mut String, arg: &TypeArg) {
        match arg {
            TypeArg::Unbounded => sb.push('?'),
            TypeArg::Extends(t) => {
                sb.push_str("? extends ");
                self.print_into(sb, t);
            }
            TypeArg::Super(t) => {
                sb.push_str("? super ");
                self.print_into(sb, t);
            }
            TypeArg::Exact(t) => self.print_into(sb, t),
        }
    }
}

fn is_object(sig: &Sig) -> bool {
    matches!(
        sig,
        Sig::Class { outer: None, name, args } if name == "java/lang/Object" && args.is_empty()
    )
}

fn base_display_name(c: char) -> &'static str {
    match c {
        'B' => "byte",
        'C' => "char",
        'D' => "double",
        'F' => "float",
        'I' => "int",
        'J' => "long",
        'S' => "short",
        'Z' => "boolean",
        'V' => "void",
        _ => "?",
    }
}
