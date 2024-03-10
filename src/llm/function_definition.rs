#![allow(dead_code)]

use std::collections::HashMap;
use std::default::Default;

use async_openai::types::{
    ChatCompletionNamedToolChoice, ChatCompletionTool, ChatCompletionToolChoiceOption,
    FunctionName, FunctionObject,
};
use serde_json::Value;
use serde_json::Value::Object;
use serde_json::{json, Map};

pub trait Definition: Send + Sync {
    fn to_json(&self) -> serde_json::Value;
}

pub struct StringDefinition {
    description: Option<String>,
}

pub struct NumberDefinition {
    description: Option<String>,
}

pub struct IntegerDefinition {
    description: Option<String>,
}

pub struct ArrayDefinition {
    description: Option<String>,
    items: Box<dyn Definition>,
}

#[derive(Default)]
pub struct ObjectDefinition {
    description: Option<String>,
    properties: HashMap<String, Box<dyn Definition>>,
    required: Vec<String>,
}

pub struct BooleanDefinition {
    description: Option<String>,
}

pub struct NullDefinition {
    description: Option<String>,
}

#[derive(Default)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: Option<String>,
    pub parameters: ObjectDefinition,
}

impl FunctionDefinition {
    pub fn to_function_object(&self) -> FunctionObject {
        FunctionObject {
            name: self.name.clone(),
            description: self.description.clone(),
            parameters: Some(self.parameters.to_json()),
        }
    }

    pub fn to_chat_completion_tool(&self) -> ChatCompletionTool {
        ChatCompletionTool {
            r#type: Default::default(),
            function: self.to_function_object(),
        }
    }

    pub fn to_tool_choice(&self) -> ChatCompletionToolChoiceOption {
        ChatCompletionToolChoiceOption::Named(ChatCompletionNamedToolChoice {
            r#type: Default::default(),
            function: FunctionName {
                name: self.name.clone(),
            },
        })
    }
}

impl Into<ChatCompletionTool> for FunctionDefinition {
    fn into(self) -> ChatCompletionTool {
        self.to_chat_completion_tool()
    }
}

impl Into<FunctionObject> for FunctionDefinition {
    fn into(self) -> FunctionObject {
        self.to_function_object()
    }
}

impl Into<ChatCompletionToolChoiceOption> for FunctionDefinition {
    fn into(self) -> ChatCompletionToolChoiceOption {
        self.to_tool_choice()
    }
}

impl Definition for StringDefinition {
    fn to_json(&self) -> serde_json::Value {
        let mut m: Map<String, Value> = Map::new();
        m.insert("type".to_string(), "string".into());
        if let Some(desc) = self.description.clone() {
            m.insert("description".to_string(), desc.into());
        }
        serde_json::Value::Object(m)
    }
}

impl Definition for NumberDefinition {
    fn to_json(&self) -> serde_json::Value {
        json!({
            "type": "number",
            "description": self.description,
        })
    }
}

impl Definition for IntegerDefinition {
    fn to_json(&self) -> serde_json::Value {
        json!({
            "type": "integer",
            "description": self.description,
        })
    }
}

impl Definition for ArrayDefinition {
    fn to_json(&self) -> serde_json::Value {
        json!({
            "type": "array",
            "description": self.description,
            "items": self.items.to_json(),
        })
    }
}

impl Definition for ObjectDefinition {
    fn to_json(&self) -> serde_json::Value {
        let mut properties = Map::new();
        for (k, v) in self.properties.iter() {
            properties.insert(k.clone(), v.to_json());
        }
        let mut obj = Map::new();
        obj.insert("type".into(), "object".into());
        if let Some(descr) = self.description.clone() {
            obj.insert("description".into(), descr.into());
        }
        obj.insert("properties".into(), Object(properties));
        Object(obj)
    }
}

impl Definition for BooleanDefinition {
    fn to_json(&self) -> serde_json::Value {
        json!({
            "type": "boolean",
            "description": self.description,
        })
    }
}

impl Definition for NullDefinition {
    fn to_json(&self) -> serde_json::Value {
        json!({
            "type": "null",
            "description": self.description,
        })
    }
}

impl ObjectDefinition {
    pub fn new(description: Option<String>) -> Self {
        ObjectDefinition {
            description,
            properties: HashMap::new(),
            required: Vec::new(),
        }
    }

    fn add_property(
        &mut self,
        name: &str,
        required: bool,
        property: impl Definition + 'static,
    ) -> &mut Self {
        self.properties.insert(name.to_string(), Box::new(property));
        if required {
            self.required.push(name.to_string());
        }
        self
    }

    pub fn add_str(&mut self, name: &str, required: bool, description: &str) -> &mut Self {
        self.add_property(
            name,
            required,
            StringDefinition {
                description: Some(description.to_string()),
            },
        )
    }

    pub fn add_num(&mut self, name: &str, required: bool, description: &str) -> &mut Self {
        self.add_property(
            name,
            required,
            NumberDefinition {
                description: Some(description.to_string()),
            },
        )
    }

    pub fn add_int(&mut self, name: &str, required: bool, description: &str) -> &mut Self {
        self.add_property(
            name,
            required,
            IntegerDefinition {
                description: Some(description.to_string()),
            },
        )
    }

    pub fn add_obj(
        &mut self,
        name: &str,
        required: bool,
        description: &str,
        objbuilder: impl FnOnce(&mut ObjectDefinition),
    ) -> &mut Self {
        let mut obj = ObjectDefinition::new(Some(description.to_string()));
        objbuilder(&mut obj);
        self.add_property(name, required, obj)
    }

    pub fn add_bool(&mut self, name: &str, required: bool, description: &str) -> &mut Self {
        self.add_property(
            name,
            required,
            BooleanDefinition {
                description: Some(description.to_string()),
            },
        )
    }

    pub fn add_null(&mut self, name: &str, description: &str) -> &mut Self {
        self.add_property(
            name,
            false,
            NullDefinition {
                description: Some(description.to_string()),
            },
        )
    }

    pub fn add_arr(
        &mut self,
        name: &str,
        required: bool,
        description: &str,
        items: impl Definition + 'static,
    ) -> &mut Self {
        self.add_property(
            name,
            required,
            ArrayDefinition {
                description: Some(description.to_string()),
                items: Box::new(items),
            },
        )
    }

    pub fn add_arr_str(&mut self, name: &str, required: bool, description: &str) -> &mut Self {
        self.add_arr(
            name,
            required,
            description,
            StringDefinition {
                description: Some(description.to_string()),
            },
        )
    }

    pub fn add_arr_num(&mut self, name: &str, required: bool, description: &str) -> &mut Self {
        self.add_arr(
            name,
            required,
            description,
            NumberDefinition {
                description: Some(description.to_string()),
            },
        )
    }

    pub fn add_arr_int(&mut self, name: &str, required: bool, description: &str) -> &mut Self {
        self.add_arr(
            name,
            required,
            description,
            IntegerDefinition {
                description: Some(description.to_string()),
            },
        )
    }

    pub fn add_arr_obj(
        &mut self,
        name: &str,
        required: bool,
        description: &str,
        objbuilder: impl FnOnce(&mut ObjectDefinition),
    ) -> &mut Self {
        let mut obj = ObjectDefinition::new(Some(description.to_string()));
        objbuilder(&mut obj);
        self.add_property(
            name,
            required,
            ArrayDefinition {
                description: Some(description.to_string()),
                items: Box::new(obj),
            },
        )
    }

    pub fn add_arr_bool(&mut self, name: &str, required: bool, description: &str) -> &mut Self {
        self.add_arr(
            name,
            required,
            description,
            BooleanDefinition {
                description: Some(description.to_string()),
            },
        )
    }
}

pub fn def_function(name: &str, description: &str) -> FunctionDefinition {
    FunctionDefinition {
        name: name.to_string(),
        description: Some(description.to_string()),
        ..Default::default()
    }
}
