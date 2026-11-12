//! Original bounded binding and assignment patterns. Patterns are flat arena
//! records, not source rewrites; runtime traversal shares ordinary reference,
//! iterator, descriptor, execution and allocation rules.
//! https://tc39.es/ecma262/#sec-destructuring-binding-patterns
//! https://tc39.es/ecma262/#sec-destructuring-assignment

use super::{
    ExprId, ExprKind, Fault, Host, Ordinary, Parser, Program, PropertyKey, Reference, Runtime,
    TokenKind, Value, MAX_ARRAY_LENGTH, MAX_EVAL_DEPTH, MAX_OBJECT_PROPERTIES,
    MAX_PROPERTY_BYTES,
};
use std::collections::HashSet;
use std::rc::Rc;

pub(super) type Id = usize;

#[derive(Clone, Copy)]
pub(super) struct Element {
    pub(super) pattern: Id,
    pub(super) initial: Option<ExprId>,
}

pub(super) enum Kind {
    Name(String),
    Target(ExprId),
    Array(Vec<Option<Element>>, Option<Id>),
    Object(Vec<(ExprId, Element)>, Option<Id>),
}

pub(super) struct Pattern {
    pub(super) kind: Kind,
    pub(super) expressions: bool,
    pub(super) at: usize,
}

#[derive(Clone, Copy)]
pub(super) enum Mode {
    Initialize(usize),
    Var(usize),
    Assign,
}

impl Program {
    pub(super) fn pattern_name(&self, id: Id) -> Option<&str> {
        match &self.patterns[id].kind {
            Kind::Name(name) => Some(name),
            Kind::Target(expression) => match &self.exprs[*expression].kind {
                ExprKind::Name(name) => Some(name),
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn pattern_names(&self, id: Id) -> Vec<String> {
        self.pattern_names_with_work(id, |_| Ok(()))
            .expect("uncharged parser pattern walk")
    }

    pub(super) fn pattern_names_with_work(
        &self,
        id: Id,
        mut charge: impl FnMut(usize) -> Result<(), Fault>,
    ) -> Result<Vec<String>, Fault> {
        let mut names = Vec::new();
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            charge(std::mem::size_of::<Pattern>())?;
            match &self.patterns[id].kind {
                Kind::Name(name) => {
                    charge(name.len())?;
                    names.push(name.clone());
                }
                Kind::Target(_) => {}
                Kind::Array(elements, rest) => {
                    charge(
                        (elements.len() + usize::from(rest.is_some())) * std::mem::size_of::<Id>(),
                    )?;
                    pending.extend(rest);
                    pending.extend(elements.iter().rev().flatten().map(|e| e.pattern));
                }
                Kind::Object(properties, rest) => {
                    charge(
                        (properties.len() + usize::from(rest.is_some()))
                            * std::mem::size_of::<Id>(),
                    )?;
                    pending.extend(rest);
                    pending.extend(properties.iter().rev().map(|(_, e)| e.pattern));
                }
            }
        }
        Ok(names)
    }
}

impl Parser {
    pub(super) fn pattern_ahead(&self) -> bool {
        let end = if self.is("[") {
            "]"
        } else if self.is("{") {
            "}"
        } else {
            return false;
        };
        let mut stack = Vec::new();
        for (offset, token) in self.tokens[self.current..].iter().enumerate() {
            match token.kind {
                TokenKind::Punct("[") => stack.push("]"),
                TokenKind::Punct("{") => stack.push("}"),
                TokenKind::Punct("(") => stack.push(")"),
                TokenKind::Punct(close @ ("]" | "}" | ")")) => {
                    if stack.pop() != Some(close) {
                        return false;
                    }
                    if stack.is_empty() {
                        if close != end {
                            return false;
                        }
                        return match self.tokens.get(self.current + offset + 1).map(|t| &t.kind) {
                            Some(TokenKind::Punct("=")) => true,
                            Some(TokenKind::Word(word))
                                if !self.allow_in && matches!(word.as_str(), "in" | "of") =>
                            {
                                true
                            }
                            _ => false,
                        };
                    }
                }
                TokenKind::End => return false,
                _ => {}
            }
        }
        false
    }

    fn add_pattern(&mut self, kind: Kind, expressions: bool, at: usize) -> Result<Id, Fault> {
        self.check_nodes()?;
        let id = self.program.patterns.len();
        self.program.patterns.push(Pattern {
            kind,
            expressions,
            at,
        });
        Ok(id)
    }

    pub(super) fn binding_pattern(&mut self, binding: bool) -> Result<Id, Fault> {
        // An identifier binding is a leaf, even at the caller's depth limit.
        // Destructuring patterns and assignment references can recurse and
        // must still enter the shared parser guard.
        if binding && !self.is("[") && !self.is("{") {
            return self.pattern_inner(binding);
        }
        self.enter()?;
        let result = self.pattern_inner(binding);
        self.depth -= 1;
        result
    }

    fn pattern_element(&mut self, binding: bool) -> Result<Element, Fault> {
        let pattern = self.binding_pattern(binding)?;
        let initial = if self.eat("=") {
            Some(self.assignment_with_in()?)
        } else {
            None
        };
        Ok(Element { pattern, initial })
    }

    fn pattern_inner(&mut self, binding: bool) -> Result<Id, Fault> {
        let at = self.at();
        if self.eat("[") {
            let mut elements = Vec::new();
            let mut rest = None;
            let mut expressions = false;
            while !self.is("]") {
                if elements.len() == MAX_ARRAY_LENGTH {
                    return Err(self.unsupported("Array pattern limit reached (8192 elements)."));
                }
                if self.eat(",") {
                    elements.push(None);
                    continue;
                }
                if self.eat("...") {
                    let id = self.binding_pattern(binding)?;
                    expressions |= self.program.patterns[id].expressions;
                    rest = Some(id);
                    if !self.is("]") {
                        return Err(self.error("An array rest element must be last, without a default or trailing comma."));
                    }
                    break;
                }
                let element = self.pattern_element(binding)?;
                expressions |=
                    element.initial.is_some() || self.program.patterns[element.pattern].expressions;
                elements.push(Some(element));
                if !self.eat(",") {
                    break;
                }
            }
            self.need("]")?;
            return self.add_pattern(Kind::Array(elements, rest), expressions, at);
        }
        if self.eat("{") {
            let mut properties = Vec::new();
            let mut rest = None;
            let mut expressions = false;
            while !self.is("}") {
                if properties.len() == MAX_OBJECT_PROPERTIES {
                    return Err(self.unsupported("Object pattern limit reached (4096 properties)."));
                }
                if self.eat("...") {
                    let id = self.binding_pattern(binding)?;
                    if !matches!(
                        self.program.patterns[id].kind,
                        Kind::Name(_) | Kind::Target(_)
                    ) {
                        return Err(self.error("An object rest property requires an identifier or assignment reference."));
                    }
                    rest = Some(id);
                    if !self.is("}") {
                        return Err(self.error("An object rest property must be last, without a default or trailing comma."));
                    }
                    break;
                }
                let key_at = self.at();
                let (key, shorthand) = if self.eat("[") {
                    expressions = true;
                    let key = self.assignment_with_in()?;
                    self.need("]")?;
                    (key, None)
                } else {
                    let (name, shorthand) = match self.token().kind.clone() {
                        TokenKind::Word(name) => {
                            let shorthand = (!super::reserved(&name)).then(|| name.clone());
                            (name, shorthand)
                        }
                        TokenKind::String(name) => (name, None),
                        TokenKind::Number(n) => (super::number_text(n), None),
                        _ => return Err(self.error("Expected an object pattern property name.")),
                    };
                    if name.len() > MAX_PROPERTY_BYTES {
                        return Err(self.unsupported("Property-name limit reached (256 bytes)."));
                    }
                    self.current += 1;
                    (
                        self.expr(ExprKind::Literal(Value::String(name)), key_at)?,
                        shorthand,
                    )
                };
                let element = if self.eat(":") {
                    self.pattern_element(binding)?
                } else if let Some(name) = shorthand {
                    self.strict_binding(&name, self.strict)?;
                    let pattern = if binding {
                        self.add_pattern(Kind::Name(name), false, key_at)?
                    } else {
                        let expression = self.expr(ExprKind::Name(name), key_at)?;
                        self.add_pattern(Kind::Target(expression), false, key_at)?
                    };
                    let initial = if self.eat("=") {
                        Some(self.assignment_with_in()?)
                    } else {
                        None
                    };
                    Element { pattern, initial }
                } else {
                    return Err(self.error("Expected ':' after the object pattern property name."));
                };
                expressions |=
                    element.initial.is_some() || self.program.patterns[element.pattern].expressions;
                properties.push((key, element));
                if !self.eat(",") {
                    break;
                }
            }
            self.need("}")?;
            return self.add_pattern(Kind::Object(properties, rest), expressions, at);
        }
        if binding {
            let name = self.name()?;
            self.strict_binding(&name, self.strict)?;
            self.add_pattern(Kind::Name(name), false, at)
        } else {
            let expression = self.left_hand_side_expression()?;
            if !self.assignable(expression) {
                return Err(self.error("Invalid destructuring assignment reference."));
            }
            if let ExprKind::Name(name) = &self.program.exprs[expression].kind {
                self.strict_binding(name, self.strict)?;
            }
            self.add_pattern(Kind::Target(expression), false, at)
        }
    }
}

impl Runtime {
    fn pattern_reference(
        &mut self,
        program: &Rc<Program>,
        pattern: Id,
        mode: Mode,
        environment: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<Option<Reference>, Fault> {
        match &program.patterns[pattern].kind {
            Kind::Name(name) => {
                let scope = match mode {
                    Mode::Initialize(scope) => scope,
                    Mode::Var(scope) => self
                        .lookup(environment, name)?
                        .filter(|found| {
                            self.environment(*found).catch_parameter.as_deref() == Some(name)
                        })
                        .unwrap_or(scope),
                    Mode::Assign => {
                        return Err(Fault::new("Invalid binding-pattern assignment mode."))
                    }
                };
                self.work(name.len())?;
                Ok(Some(Reference::Binding(scope, name.clone())))
            }
            Kind::Target(expression) => self
                .reference(program, *expression, environment, this, host)
                .map(Some),
            _ => Ok(None),
        }
    }

    fn pattern_write(
        &mut self,
        reference: Reference,
        value: Value,
        mode: Mode,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        if let Mode::Initialize(_) = mode {
            let Reference::Binding(scope, name) = reference else {
                return Err(Fault::new("Invalid binding initialization reference."));
            };
            self.write_binding(scope, &name, value, true, host)
        } else {
            self.write_reference(reference, value, host)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn initialize_pattern(
        &mut self,
        program: &Rc<Program>,
        pattern: Id,
        value: Value,
        mode: Mode,
        environment: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        self.step()?;
        if self.eval_depth >= MAX_EVAL_DEPTH {
            return Err(Fault::new("Evaluation nesting limit reached (128)."));
        }
        self.eval_depth += 1;
        let result = (|| {
            if let Some(reference) =
                self.pattern_reference(program, pattern, mode, environment, this, host)?
            {
                return self.pattern_write(reference, value, mode, host);
            }
            match &program.patterns[pattern].kind {
                Kind::Array(elements, rest) => self.initialize_array_pattern(
                    program,
                    elements,
                    *rest,
                    value,
                    mode,
                    environment,
                    this,
                    host,
                ),
                Kind::Object(properties, rest) => self.initialize_object_pattern(
                    program,
                    properties,
                    *rest,
                    value,
                    mode,
                    environment,
                    this,
                    host,
                ),
                _ => unreachable!(),
            }
        })();
        self.eval_depth -= 1;
        result.map_err(|error| error.at(&program.source, program.patterns[pattern].at))
    }

    #[allow(clippy::too_many_arguments)]
    fn pattern_element_value(
        &mut self,
        program: &Rc<Program>,
        element: Element,
        reference: Option<Reference>,
        mut value: Value,
        mode: Mode,
        environment: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        if let (Value::Undefined, Some(initial)) = (&value, element.initial) {
            value = self.parameter_initializer(
                program,
                initial,
                program.pattern_name(element.pattern),
                environment,
                this,
                host,
            )?;
        }
        if let Some(reference) = reference {
            self.pattern_write(reference, value, mode, host)
        } else {
            self.initialize_pattern(
                program,
                element.pattern,
                value,
                mode,
                environment,
                this,
                host,
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn initialize_array_pattern(
        &mut self,
        program: &Rc<Program>,
        elements: &[Option<Element>],
        rest: Option<Id>,
        value: Value,
        mode: Mode,
        environment: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        let method = self.iterator_method(&value, host)?.ok_or_else(|| {
            Fault::exception("TypeError", "An array pattern requires an iterable value.")
        })?;
        let mut iterator = self.iterator_from_method(value, method, host)?;
        let result = (|| {
            for element in elements {
                self.step()?;
                if let Some(element) = element {
                    let reference = self.pattern_reference(
                        program,
                        element.pattern,
                        mode,
                        environment,
                        this,
                        host,
                    )?;
                    let value = self
                        .iterator_next(&mut iterator, host)?
                        .unwrap_or(Value::Undefined);
                    self.pattern_element_value(
                        program,
                        *element,
                        reference,
                        value,
                        mode,
                        environment,
                        this,
                        host,
                    )?;
                } else {
                    self.iterator_skip(&mut iterator, host)?;
                }
            }
            if let Some(rest) = rest {
                let reference =
                    self.pattern_reference(program, rest, mode, environment, this, host)?;
                let array = self.new_object(Some(0))?;
                let mut index = 0usize;
                while let Some(value) = self.iterator_next(&mut iterator, host)? {
                    if index == self.limits.heap.array_length {
                        return Err(Fault::new(format!("Array length limit reached ({}).", self.limits.heap.array_length)));
                    }
                    self.store_property(array, index.to_string(), value, Some((true, true, true)))?;
                    index += 1;
                }
                self.pattern_element_value(
                    program,
                    Element {
                        pattern: rest,
                        initial: None,
                    },
                    reference,
                    Value::Object(array),
                    mode,
                    environment,
                    this,
                    host,
                )?;
            }
            Ok(())
        })();
        self.iterator_close(&mut iterator, result, host)
    }

    #[allow(clippy::too_many_arguments)]
    fn initialize_object_pattern(
        &mut self,
        program: &Rc<Program>,
        properties: &[(ExprId, Element)],
        rest: Option<Id>,
        value: Value,
        mode: Mode,
        environment: usize,
        this: &Value,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        Self::require_property_receiver(&value)?;
        let mut excluded = HashSet::new();
        for (key, element) in properties {
            self.step()?;
            let key_value = self.eval(program, *key, environment, this, host)?;
            let key = self.property_key(key_value, host)?;
            let reference =
                self.pattern_reference(program, element.pattern, mode, environment, this, host)?;
            let receiver = self.copy_value(&value)?;
            let next = self.read_reference(&key.clone().reference(receiver), host)?;
            self.pattern_element_value(
                program,
                *element,
                reference,
                next,
                mode,
                environment,
                this,
                host,
            )?;
            if rest.is_some() {
                self.work(match &key {
                    PropertyKey::String(name) => name.len(),
                    PropertyKey::Symbol(_) => std::mem::size_of::<usize>(),
                })?;
                excluded.insert(key);
            }
        }
        if let Some(rest) = rest {
            let reference = self.pattern_reference(program, rest, mode, environment, this, host)?;
            let object = self.pattern_rest_object(&value, &excluded, host)?;
            self.pattern_element_value(
                program,
                Element {
                    pattern: rest,
                    initial: None,
                },
                reference,
                object,
                mode,
                environment,
                this,
                host,
            )?;
        }
        Ok(())
    }

    fn pattern_rest_object(
        &mut self,
        value: &Value,
        excluded: &HashSet<PropertyKey>,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let object = self.new_object(None)?;
        self.copy_data_properties(object, value, excluded, host)?;
        Ok(Value::Object(object))
    }

    // Object rest and literal spread use the same snapshot of own keys, live
    // descriptor/Get reads, and own data writes. Nullish spread is empty; an
    // object binding already rejected nullish values before reaching this path.
    pub(super) fn copy_data_properties(
        &mut self,
        object: usize,
        value: &Value,
        excluded: &HashSet<PropertyKey>,
        host: &mut dyn Host,
    ) -> Result<(), Fault> {
        if matches!(value, Value::Host(_)) {
            return Err(Fault::new(
                "Native-host object rest/spread is not implemented.",
            ));
        }
        if let Value::String(text) = value {
            self.work(text.len().saturating_mul(2))?;
            let units = text.encode_utf16().collect::<Vec<_>>();
            if units.len() > self.limits.heap.object_properties {
                return Err(Fault::new(
                    format!("Property enumeration limit reached ({} keys).", self.limits.heap.object_properties),
                ));
            }
            for (index, unit) in units.iter().enumerate() {
                self.step()?;
                let key = index.to_string();
                if !excluded.contains(&PropertyKey::String(key.clone())) {
                    let next = Value::String(self.units_string(std::slice::from_ref(unit))?);
                    self.store_property(object, key, next, Some((true, true, true)))?;
                }
            }
        } else if let Some(ordinary) = Ordinary::from_value(value) {
            let source = self.ordinary_id(ordinary)?;
            let keys = self.own_string_keys(source, false)?;
            let symbols = self.own_symbol_keys(source, false)?;
            if keys.len().saturating_add(symbols.len()) > self.limits.heap.object_properties {
                return Err(Fault::new(
                    format!("Property enumeration limit reached ({} keys).", self.limits.heap.object_properties),
                ));
            }
            for key in keys {
                self.step()?;
                if excluded.contains(&PropertyKey::String(key.clone()))
                    || !self.own_enumerable(source, &key)?
                {
                    continue;
                }
                let receiver = self.copy_value(value)?;
                let next =
                    self.read_reference(&Reference::Property(receiver, key.clone()), host)?;
                self.store_property(object, key, next, Some((true, true, true)))?;
            }
            for key in symbols {
                self.step()?;
                if excluded.contains(&PropertyKey::Symbol(key))
                    || !self.objects[source]
                        .symbol_properties
                        .get(&key)
                        .is_some_and(|p| p.enumerable)
                {
                    continue;
                }
                let receiver = self.copy_value(value)?;
                let next = self.get_symbol_property(receiver, key, host)?;
                self.store_symbol_property(object, key, next, Some((true, true, true)))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct NoHost {
        reads: usize,
    }
    impl Host for NoHost {
        fn get(&mut self, _: u32, name: &str) -> Result<Value, String> {
            self.reads += 1;
            if name == "allowed" {
                Ok(Value::Number(7.0))
            } else {
                Err("No native read.".into())
            }
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("No native writes.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("No native calls.".into())
        }
        fn allow_eval(&mut self) -> Result<bool, String> {
            Ok(true)
        }
    }
    fn succeeds(source: &str) {
        assert_eq!(
            Runtime::for_test()
                .execute(source, &mut NoHost::default())
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn patterns_declarations_nested_defaults_rest_and_strings() {
        succeeds(
            r#"
let [a,,b=4,...tail]=[1,2,undefined,5,6];
let {x:y,z:{n=7}={},...rest}={x:3,k:9};
if([a,b,tail.join(),y,n,rest.k].join('|')!=='1|4|5,6|3|7|9')throw 'nested values';
var {p:{q=8}={}}={p:{}};if(q!==8)throw 'var nested binding';
let [point,...letters]='\uD83D\uDE3Ax';if(point!=='\uD83D\uDE3A'||letters.join()!=='x')throw 'string iteration';
let {0:first,...text}='abc';if(first!=='a'||text[1]!=='b'||text[2]!=='c'||Object.keys(text).join()!=='1,2')throw 'string rest';
let {}=3;let []=[];let [...empty]=[];if(empty.length!==0)throw 'empty rest';'ok';
"#,
        );
    }

    #[test]
    fn patterns_object_rest_uses_live_own_descriptors_and_symbol_exclusions() {
        succeeds(
            r#"
var excluded=Symbol('excluded'),kept=Symbol('kept'),log='';
var source={get a(){log+='a';return 1;},get b(){log+='b';delete source.c;source.extra=9;return 2;},c:3};
Object.setPrototypeOf(source,{inherited:5});source[excluded]=6;source[kept]=7;
Object.defineProperty(source,'hidden',{value:8});
let {a,[excluded]:ignored,...rest}=source;
if(a!==1||ignored!==6||rest.b!==2||rest[kept]!==7||rest[excluded]!==undefined||log!=='ab')throw 'rest values';
if(Object.keys(rest).join()!=='b'||Object.getOwnPropertySymbols(rest).length!==1||rest.inherited!==undefined||rest.hidden!==undefined)throw 'own keys';
var descriptor=Object.getOwnPropertyDescriptor(rest,'b');
if(descriptor.value!==2||!descriptor.writable||!descriptor.enumerable||!descriptor.configurable||descriptor.get!==undefined)throw 'rest data descriptor';'ok';
"#,
        );
    }

    #[test]
    fn patterns_assignment_preserves_rhs_and_target_evaluation_order() {
        succeeds(
            r#"
var log='',object={},source={get x(){log+='g';return 11;}},a;
function target(){log+='t';return object;}
var result=({x:target().value}=source);
if(result!==source||object.value!==11||log!=='tg')throw 'object target order';
var input=[12,13];if(([a,object.value]=input)!==input||a!==12||object.value!==13)throw 'assignment result';
var iterable={};iterable[Symbol.iterator]=function(){log+='i';return {next(){log+='n';return {value:4,done:false};},return(){log+='r';return {};}};};
log='';[target().value]=iterable;if(log!=='itnr'||object.value!==4)throw 'array target before next';
var first,second;[first=function(){},second=()=>1]=[];if(first.name!=='first'||second.name!=='second')throw 'assignment default names';
({x:object.value=7}={});if(object.value!==7)throw 'member default';'ok';
"#,
        );
    }

    #[test]
    fn patterns_iterator_elisions_do_not_read_value_and_normal_completion_closes() {
        succeeds(
            r#"
var log='',iterable={};iterable[Symbol.iterator]=function(){log+='i';var n=0;return {next(){log+='n';return {get done(){log+='d';return false;},get value(){log+='v';return ++n;}};},return(){log+='r';return {};}};};
let [,value]=iterable;if(value!==1||log!=='indndvr')throw 'elision value access';
log='';let []=iterable;if(log!=='ir')throw 'empty pattern close';
var finite={};finite[Symbol.iterator]=function(){return {next(){return {done:true,get value(){throw 'unreachable';}};},return(){throw 'unreachable';}};};
let [a=3,b=4,...tail]=finite;if(a!==3||b!==4||tail.length!==0)throw 'already done';'ok';
"#,
        );
    }

    #[test]
    fn patterns_iterator_abrupt_completions_close_only_when_required() {
        succeeds(
            r#"
var marker={},log='',iterable={};
function make(mode){var source={};source[Symbol.iterator]=function(){return {next(){log+='n';if(mode==='next')throw marker;return {get done(){log+='d';if(mode==='done')throw marker;return false;},get value(){log+='v';if(mode==='value')throw marker;return undefined;}};},return(){log+='r';throw 99;}};};return source;}
for(var mode of ['next','done','value']){log='';try{let [a]=make(mode);throw 'missing throw';}catch(e){if(e!==marker)throw e;}if(log.indexOf('r')!==-1)throw 'closed failed step';}
function fail(){log+='f';throw marker;}log='';try{let [a=fail()]=make('default');}catch(e){if(e!==marker)throw 'close replaced original';}
if(log!=='ndvfr')throw 'default close';
function target(){log+='t';throw marker;}log='';try{[target().x]=make('target');}catch(e){if(e!==marker)throw e;}
if(log!=='tr')throw 'target close before next';
iterable[Symbol.iterator]=function(){return {next(){return {value:1,done:false};},return(){return 3;}};};
var caught=false;try{let [a]=iterable;}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'invalid return result';'ok';
"#,
        );
    }

    #[test]
    fn patterns_parameter_environments_distinguish_plain_patterns_from_expressions() {
        succeeds(
            r#"
function plain({x}){var x;arguments[0]={x:8};return x;}
if(plain({x:3})!==3||plain.length!==1)throw 'plain parameter/arguments';
function defaults({x=2,read=()=>x}){var x=9;return [x,read()].join();}
if(defaults({})!=='9,2'||defaults.length!==1)throw 'nested default parameter environment';
function computed({[(saved=()=>x,'x')]:x}){var x=9;return [x,saved()].join();}
var saved;if(computed({x:3})!=='9,3')throw 'computed parameter environment';
function named({arguments}){return arguments;}if(named({arguments:6})!==6)throw 'arguments pattern binding';
function rest(...[a=4,b]){return [a,b].join();}if(rest(undefined,5)!=='4,5'||rest.length!==0)throw 'rest pattern';
function objectRest(...{length}){return length;}if(objectRest(1,2)!==2)throw 'object rest parameter';
var arrow=({x=3}={},[y]=[4])=>x+y;if(arrow()!==7||arrow.length!==0)throw 'arrow patterns';
var dynamic=Function('{x=2}','return x;');if(dynamic({})!==2||dynamic.length!==1)throw 'dynamic pattern';'ok';
"#,
        );
    }

    #[test]
    fn patterns_loops_and_catch_keep_tdz_closures_and_finally() {
        succeeds(
            r#"
var callbacks=[];for(let [a,b] of [[1,2],[3,4]])callbacks.push(()=>a+b);
if(callbacks[0]()!==3||callbacks[1]()!==7)throw 'for of closures';
var values=[];for(let {length:n} in {ab:1,xyz:2})values.push(n);if(values.join()!=='2,3')throw 'for in pattern';
var left,right;for([left,right] of [[4,5]]){}if(left!==4||right!==5)throw 'assignment loop';
for(var [v] of [[6]]){}if(v!==6)throw 'var loop';
callbacks=[];for(let [i]=[0];i<2;i++)callbacks.push(()=>i);if(callbacks[0]()!==0||callbacks[1]()!==1)throw 'for let closures';
var caught='';try{for(let [outer] of outer){}}catch(e){caught=e.name;}if(caught!=='ReferenceError')throw 'loop rhs TDZ';
var finalized=false;try{try{throw null;}catch({x}){throw 'unreachable';}finally{finalized=true;}}catch(e){if(e.name!=='TypeError')throw e;}
if(!finalized)throw 'catch initialization finally';
try{throw {x:7,nested:[8]};}catch({x,nested:[other]}){if(x+other!==15)throw 'catch values';}
var simple=1;try{throw 2;}catch(simple){var simple=3;if(simple!==3)throw 'simple catch var';}if(simple!==1)throw 'simple catch outer';'ok';
"#,
        );
    }

    #[test]
    fn patterns_tdz_const_and_default_function_names() {
        succeeds(
            r#"
var errors=[];try{let [a=a]=[];}catch(e){errors.push(e.name);}try{let {x:y=z,z}={z:1};}catch(e){errors.push(e.name);}
try{function f([x=x]){}f([]);}catch(e){errors.push(e.name);}try{let {}=null;}catch(e){errors.push(e.name);}
try{let []=undefined;}catch(e){errors.push(e.name);}try{const [fixed]=[3];[fixed]=[4];}catch(e){errors.push(e.name);}
if(errors.join()!=='ReferenceError,ReferenceError,ReferenceError,TypeError,TypeError,TypeError')throw 'pattern errors';
let [first=function(){},second=()=>1,third=function named(){}]=[];
let {callback=function(){}}={};if([first.name,second.name,third.name,callback.name].join()!=='first,second,named,callback')throw 'binding default names';
let [a=2,b=a+1]=[];if(a!==2||b!==3)throw 'left to right';'ok';
"#,
        );
    }

    #[test]
    fn patterns_early_errors_are_atomic_and_preserve_simple_rest_negatives() {
        for tail in [
            "let [...a,]=[];",
            "let {...{a}}={};",
            "let [a];",
            "var {a};",
            "let [a,a]=[];",
            "function f([a],a){}",
            "function f({a}){'use strict';}",
            "function f([a]){let a;}",
            "try{}catch({a}){var a;}",
            "try{}catch([a,a]){}",
            "let [object.x]=[];",
            "var a;[...a,]=[];",
            "var a;({x:a+=1}={});",
            "for(let [a] of []){var a;}",
            "var a;[a]++;",
            "var a;({a})={};",
            "let {a,...b,c}={};",
            "let {a,...b={}}={};",
            "let [a,...b=[]]=[];",
        ] {
            let mut runtime = Runtime::for_test();
            let mut host = NoHost::default();
            runtime.execute("var effect=false;", &mut host).unwrap();
            let source = format!("effect=true;{tail}");
            let error = match Parser::parse(&source, &mut 0) {
                Err(error) => error,
                Ok(_) => panic!("Accepted invalid pattern syntax: {tail}"),
            };
            assert!(
                matches!(
                    error.exception.as_deref(),
                    Some(super::super::Exception::Error("SyntaxError"))
                ),
                "{tail}: {error:?}"
            );
            runtime.execute(&source, &mut host).unwrap_err();
            runtime.begin_job();
            assert_eq!(
                runtime.execute("effect;", &mut host).unwrap().to_text(),
                "false",
                "{tail}"
            );
        }
    }

    #[test]
    fn patterns_infinite_rest_preserves_fatal_bounds_without_cleanup() {
        let mut runtime = Runtime::for_test();
        let mut host = NoHost::default();
        runtime.execute("var caught=false,finalized=false,closed=false;var result={value:1,done:false},iterable={};iterable[Symbol.iterator]=function(){return {next(){return result;},return(){closed=true;return {};}};};", &mut host).unwrap();
        runtime.begin_job();
        let error = runtime
            .execute(
                "try{let [...tail]=iterable;}catch(e){caught=true;}finally{finalized=true;}",
                &mut host,
            )
            .unwrap_err();
        assert!(error.contains("limit"), "{error}");
        assert_eq!(runtime.eval_depth, 0);
        assert_eq!(runtime.call_depth, 0);
        assert!(runtime.objects.len() <= super::super::MAX_OBJECTS);
        assert!(runtime.properties <= super::super::MAX_PROPERTIES);
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("!caught&&!finalized&&!closed;", &mut host)
                .unwrap()
                .to_text(),
            "true"
        );
    }

    #[test]
    fn patterns_native_reads_keep_host_boundary_and_rest_is_explicitly_unsupported() {
        let mut runtime = Runtime::for_test();
        let mut host = NoHost::default();
        runtime.define_global("native", Value::Host(7)).unwrap();
        assert_eq!(
            runtime
                .execute("let {allowed}=native;allowed;", &mut host)
                .unwrap()
                .to_text(),
            "7"
        );
        assert_eq!(host.reads, 1);
        runtime
            .execute("var caught=false,finalized=false;", &mut host)
            .unwrap();
        let error = runtime
            .execute(
                "try{let {...rest}=native;}catch(e){caught=true;}finally{finalized=true;}",
                &mut host,
            )
            .unwrap_err();
        assert!(error.contains("Native-host object rest"), "{error}");
        assert_eq!(host.reads, 1);
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("!caught&&!finalized;", &mut host)
                .unwrap()
                .to_text(),
            "true"
        );
    }

    #[test]
    fn patterns_share_ast_retention_and_parser_nesting_limits() {
        let mut runtime = Runtime::for_test();
        let program = runtime
            .register_program("let [{a:b=3},...rest]=[];", false)
            .unwrap();
        assert!(!program.patterns.is_empty());
        assert_eq!(
            program.node_count(),
            program.exprs.len()
                + program.stmts.len()
                + program.templates.len()
                + program.patterns.len()
        );
        assert_eq!(runtime.retained_ast, program.node_count());
        let source = format!("var effect=1;let {}a{}=[];", "[".repeat(80), "]".repeat(80));
        let mut runtime = Runtime::for_test();
        let mut host = NoHost::default();
        let error = runtime.execute(&source, &mut host).unwrap_err();
        assert!(error.contains("nesting limit"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute("typeof effect;", &mut host)
                .unwrap()
                .to_text(),
            "undefined"
        );
    }
}
