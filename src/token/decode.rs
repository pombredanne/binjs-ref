//! With the help of an underlying `TokenReader`, decode a stream of bytes
//! to a JSON matching a specific grammar.

use ast::grammar::*;
use token::io::*;

use serde_json;
use serde_json::Value;

type Object = serde_json::Map<String, Value>;

pub enum Error {
    UnexpectedValue(String),
    TokenReaderError, // FIXME: Improve this
    NoSuchInterface(String),
    NoSuchRefinement(String),
    NoSuchKind(String),
    NoSuchField(String),
    InvalidValue(String),
}

pub struct Decoder<'a, E> where E: TokenReader {
    extractor: E,
    grammar: &'a Syntax,
}

impl<'a, E> Decoder<'a, E> where E: TokenReader {
    pub fn new(extractor: E, grammar: &'a Syntax) -> Self {
        Decoder {
            extractor,
            grammar
        }
    }
    pub fn decode(&mut self, kind: &Type) -> Result<Value, Error> {
        use ast::grammar::Type::*;
        match *kind {
            Array(ref kind) => {
                let (len, extractor) = self.extractor.list()
                    .map_err(|_| Error::TokenReaderError)?;
                let mut decoder = Decoder::new(extractor, self.grammar);
                let mut values = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    values.push(decoder.decode(kind)?);
                }
                Ok(Value::Array(values))
            }
            Obj(ref structure) => {
                // At this stage, since there is no inheritance involved, use the built-in mapping.
                let extractor = self.extractor.untagged_tuple()
                    .map_err(|_| Error::TokenReaderError)?;
                let mut decoder = Decoder::new(extractor, self.grammar);
                let mut object = Object::new();
                for field in structure.fields() {
                    let item = decoder.decode(field.type_())?;
                    object.insert(field.name().to_string().clone(), item);
                }
                Ok(Value::Object(object))
            }
            String => {
                let string = self.extractor.string()
                    .map_err(|_| Error::TokenReaderError)?
                    .ok_or_else(|| Error::UnexpectedValue("(no string)".to_string()))?;
                Ok(Value::String(string))
            }
            Enum(ref enum_) => {
                let string = self.extractor.string()
                    .map_err(|_| Error::TokenReaderError)?;
                match string {
                    None if enum_.or_null() => Ok(Value::Null),
                    None => Err(Error::UnexpectedValue("(no string)".to_string())),
                    Some(s) => {
                        for candidate in enum_.strings() {
                            if candidate == &s {
                                return Ok(Value::String(s))
                            }
                        }
                        Err(Error::UnexpectedValue(s))
                    }
                }
            }
            Interfaces(ref interfaces) => {
                let (kind_name, mapped_field_names, extractor) = self.extractor.tagged_tuple()
                    .map_err(|_| Error::TokenReaderError)?;

                // We have a kind, so we know how to parse the data. We just need
                // to make sure that we expected this interface here.
                let kind = self.grammar.get_kind(&kind_name)
                    .ok_or_else(|| Error::NoSuchKind(kind_name))?;

                if let Some(interface) = self.grammar.get_interface_by_kind(&kind) {
                    if self.grammar
                         .get_ancestors_by_name_including_self(interface.name())
                         .unwrap()
                         .iter()
                         .find(|ancestor|
                             interfaces.iter()
                                 .find(|candidate| candidate == ancestor)
                                 .is_some()
                         ).is_none()
                    {
                         return Err(Error::NoSuchRefinement(kind.to_string().clone()))
                    }

                    // Read the fields **in the order** in which they appear in the stream.
                    let mut decoder = Decoder::new(extractor, self.grammar);
                    let mut object = Object::new();
                    for field in mapped_field_names.as_ref().iter() {
                        let item = decoder.decode(field.type_())?;
                        object.insert(field.name().to_string().clone(), item);
                    }
                    Ok(Value::Object(object))
                } else {
                    Err(Error::NoSuchKind(kind.to_string().clone()))
                }
            }
            Boolean => {
                let value = self.extractor.bool()
                    .map_err(|_| Error::InvalidValue("bool".to_string()))?;
                Ok(Value::Bool(value))
            }
            Number => {
                let value = self.extractor.float()
                    .map_err(|_| Error::InvalidValue("float".to_string()))?;
                Ok(json!(value))
            }
        }
    }
}