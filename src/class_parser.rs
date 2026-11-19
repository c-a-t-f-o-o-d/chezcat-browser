//! Original finite base-class syntax. Method bodies reuse the bounded deferred
//! function parser; no class source is rewritten or executed during parsing.
//! https://tc39.es/ecma262/#sec-class-definitions
use super::{number_text, object_literals::Kind, ExprId, ExprKind, Fault, Parser, Template, TokenKind};
use std::collections::HashSet;

#[derive(Clone)]
pub(super) struct Class {
    pub(super) name: Option<String>,
    pub(super) constructor: usize,
    pub(super) heritage: Option<ExprId>,
    pub(super) members: Vec<Member>,
}

#[derive(Clone)]
pub(super) struct Member {
    pub(super) key: MemberKey,
    pub(super) template: usize,
    pub(super) kind: Kind,
    pub(super) is_static: bool,
}

#[derive(Clone)]
pub(super) enum MemberKey {
    Literal(String),
    Computed(ExprId),
}

impl Parser {
    pub(super) fn class_expression(&mut self, start: usize, declaration: bool) -> Result<ExprKind, Fault> {
        self.enter()?;
        let previous_strict = std::mem::replace(&mut self.strict, true);
        let result = self.class_definition(start, declaration);
        self.strict = previous_strict;
        self.depth -= 1;
        result
    }

    fn class_definition(&mut self, start: usize, declaration: bool) -> Result<ExprKind, Fault> {
        self.need("class")?;
        self.class_node_capacity(0)?;
        let name = if declaration || !(self.is("{") || self.is("extends")) {
            if let TokenKind::Word(name) | TokenKind::EscapedWord(name) = &self.token().kind {
                if name.len() > self.capacity.property_bytes {
                    return Err(self.unsupported("Class-name limit reached."));
                }
            }
            let name = self.name()?;
            self.strict_binding(&name, true)?;
            Some(name)
        } else {
            None
        };
        let heritage = if self.eat("extends") { Some(self.left_hand_side_expression()?) } else { None };
        self.need("{")?;
        let mut members = Vec::new();
        let mut constructor = None;
        let mut member_count = 0usize;
        while !self.is("}") {
            if self.eat(";") { continue; }
            if matches!(self.token().kind, TokenKind::End) {
                return Err(self.error("Unterminated class body."));
            }
            if member_count == self.heap.object_properties {
                return Err(self.unsupported(format!("Class member limit reached ({}).", self.heap.object_properties)));
            }
            member_count += 1;
            self.class_node_capacity(members.len())?;
            let is_static = self.is("static") && !self.next_is("(");
            if is_static {
                self.current += 1;
                if self.is("{") {
                    return Err(self.unsupported("Class static blocks are not implemented."));
                }
            }
            // The ClassElement `static` modifier is outside MethodDefinition's
            // source range; an accessor's get/set token belongs to that range.
            let method_start = self.at();
            let kind = if (self.is("get") || self.is("set")) && !self.next_is("(") {
                if self.eat("get") { Kind::Getter } else { self.need("set")?; Kind::Setter }
            } else {
                Kind::Method
            };
            let asynchronous = kind == Kind::Method && self.is("async") && !self.next_is("(")
                && !self.tokens[self.current + 1].newline();
            if asynchronous {
                self.current += 1;
            }
            if self.is("*") {
                return Err(self.unsupported("Generator class methods are not implemented."));
            }
            let key = if self.eat("[") {
                let prior_awaits = self.await_count;
                let expression = self.assignment_with_in()?;
                if self.await_count != prior_awaits {
                    return Err(self.unsupported("Await in a computed class key is not implemented."));
                }
                self.need("]")?;
                MemberKey::Computed(expression)
            } else {
                MemberKey::Literal(self.class_literal_key()?)
            };
            let literal = match &key { MemberKey::Literal(name) => Some(name.as_str()), MemberKey::Computed(_) => None };
            if is_static && literal == Some("prototype") {
                return Err(self.error("A static class member cannot be named prototype."));
            }
            let is_constructor = !is_static && literal == Some("constructor");
            if is_constructor && kind != Kind::Method {
                return Err(self.error("A class constructor cannot be an accessor."));
            }
            if is_constructor && asynchronous {
                return Err(self.error("A class constructor cannot be async."));
            }
            if is_constructor && constructor.is_some() {
                return Err(self.error("A class cannot have duplicate constructors."));
            }
            if !self.is("(") {
                return Err(self.unsupported("Class fields are not implemented; expected a method parameter list."));
            }
            let template = self.function_with_super(None, method_start, asynchronous, true, is_constructor && heritage.is_some())?;
            let syntax = &self.program.templates[template];
            let names = syntax.parameter_names(&self.program);
            if names.iter().collect::<HashSet<_>>().len() != names.len() {
                return Err(self.error("Duplicate class method parameters are forbidden."));
            }
            for name in &names { self.strict_binding(name, true)?; }
            if kind == Kind::Getter && !syntax.params.is_empty() {
                return Err(self.error("A class getter must have no parameters."));
            }
            if kind == Kind::Setter && (syntax.params.len() != 1 || syntax.rest || syntax.trailing_parameter_comma) {
                return Err(self.error("A class setter must have exactly one parameter, without rest or a trailing comma."));
            }
            let syntax = &mut self.program.templates[template];
            syntax.constructible = is_constructor;
            syntax.class_constructor = is_constructor;
            syntax.class_element = true;
            syntax.derived_constructor = is_constructor && heritage.is_some();
            syntax.default_constructor = false;
            if is_constructor {
                constructor = Some(template);
            } else {
                self.class_node_capacity(members.len().saturating_add(1))?;
                members.try_reserve(1).map_err(|_| self.unsupported("Unable to reserve bounded class member storage."))?;
                members.push(Member { key, template, kind, is_static });
            }
        }
        self.need("}")?;
        let end = self.tokens[self.current - 1].end();
        self.class_node_capacity(members.len())?;
        let constructor = if let Some(constructor) = constructor {
            let syntax = &mut self.program.templates[constructor];
            syntax.source_start = start;
            syntax.source_end = end;
            constructor
        } else {
            let constructor = self.program.templates.len();
            self.program.templates.try_reserve(1).map_err(|_| self.unsupported("Unable to reserve bounded class constructor storage."))?;
            self.program.templates.push(Template {
                source_start: start, source_end: end, dynamic_source: false,
                arrow: false, asynchronous: false, await_plan: Vec::new(),
                constructible: true, class_constructor: true, class_element: true,
                derived_constructor: heritage.is_some(), default_constructor: true, strict: true,
                name: None, params: Vec::new(), rest: false,
                trailing_parameter_comma: false, body: Vec::new(),
            });
            constructor
        };
        let index = self.program.classes.len();
        self.program.classes.try_reserve(1).map_err(|_| self.unsupported("Unable to reserve bounded class syntax storage."))?;
        self.program.class_members += members.len();
        self.program.classes.push(Class { name, constructor, heritage, members });
        Ok(ExprKind::Class(index))
    }

    pub(super) fn next_is(&self, text: &str) -> bool {
        match self.tokens.get(self.current + 1).map(|token| &token.kind) {
            Some(TokenKind::Word(word)) => word == text,
            Some(TokenKind::Punct(punct)) => *punct == text,
            _ => false,
        }
    }

    fn class_literal_key(&mut self) -> Result<String, Fault> {
        let name = match &self.token().kind {
            TokenKind::Word(name) | TokenKind::EscapedWord(name) | TokenKind::String(name) => {
                if name.len() > self.capacity.property_bytes {
                    return Err(self.unsupported("Class property-name limit reached."));
                }
                name.clone()
            }
            TokenKind::Number(number) => {
                let name = number_text(*number);
                if name.len() > self.capacity.property_bytes {
                    return Err(self.unsupported("Class property-name limit reached."));
                }
                name
            }
            _ => return Err(self.error("Expected a literal public class method name.")),
        };
        self.current += 1;
        Ok(name)
    }

    // Pending members are not published in Program yet. Include that local
    // list plus the class node before reserve/push, including empty classes.
    fn class_node_capacity(&self, pending_members: usize) -> Result<(), Fault> {
        if self.program.node_count().saturating_add(pending_members).saturating_add(1) > self.capacity.ast_nodes {
            return Err(self.unsupported(format!("AST node limit reached ({}).", self.capacity.ast_nodes)));
        }
        Ok(())
    }
}
