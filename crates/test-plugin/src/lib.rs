use crate::{
    bindings::{
        bedrock_ecs::plugin::server,
        bedrock_ecs::plugin::system::{AccessDescriptor, SystemManifest},
        exports::bedrock_ecs::plugin::{
            metadata,
            system::{self, System},
        },
    },
    local::Local,
};

pub mod local;

mod bindings {
    wit_bindgen::generate!({
        path: ["./wit"],
        world: "component:plugin/plugin",
        with: {
            "bedrock-ecs:plugin/server": generate,
            "bedrock-ecs:plugin/metadata": generate,
            "bedrock-ecs:plugin/system": generate,
        }
    });

    use super::Plugin;
    export!(Plugin);
}

struct Plugin;

impl system::Guest for Plugin {
    type System = SystemWrapper;
}

impl metadata::Guest for Plugin {
    fn get_manifest() -> metadata::Manifest {
        let version = server::get_version();
        println!("server version is: {version:?}");

        metadata::Manifest {
            name: String::from("plugin"),
            version: String::from("0.1.0"),
        }
    }

    fn init() {
        let manifest = SystemManifest {
            name: String::from("wasm_system"),
            access: vec![AccessDescriptor::Local],
        };

        let id = server::register_system(&manifest);
        println!("id {id} assigned");
    }
}

struct SystemWrapper;

impl system::GuestSystem for SystemWrapper {
    fn call(&self) {
        todo!()
    }
}

// impl system::GuestSystemCallable for SystemWrapper {
//     fn manifest(&self) -> system_export::Manifest {
//         todo!()
//     }

//     fn call(&self) {}
// }

pub fn system_name(local: Local<usize>) {
    println!("local is: {}", *local);
}
