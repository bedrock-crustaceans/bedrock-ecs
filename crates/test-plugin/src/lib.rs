use crate::{
    bindings::{
        bedrock_ecs::plugin::{
            host,
            system::{self, AccessDescriptor, AccessType, SystemManifest},
        },
        exports::bedrock_ecs::plugin::metadata::{self, PluginManifest},
    },
    local::Local,
};

pub mod local;

mod bindings {
    wit_bindgen::generate!({
        path: ["./wit"],
        world: "component:plugin/plugin",
        with: {
            "bedrock-ecs:plugin/types": generate,
            "bedrock-ecs:plugin/system": generate,
            "bedrock-ecs:plugin/host": generate,
            "bedrock-ecs:plugin/metadata": generate,
        }
    });

    use super::Plugin;
    export!(Plugin);
}

struct Plugin;

impl metadata::Guest for Plugin {
    fn init() {
        println!("host version is {}", host::get_version());

        let system_id = host::register_system(&SystemManifest {
            name: String::from("wasm_test_system"),
            access: vec![],
        })
        .unwrap();

        println!("system id is: {system_id}");
    }

    fn get_manifest() -> PluginManifest {
        PluginManifest {
            name: String::from("test-plugin"),
            version: String::from("0.1.0"),
        }
    }

    fn deinit() {}
}
