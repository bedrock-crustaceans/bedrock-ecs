use crate::bindings::{
    bedrock_ecs::plugin::server,
    exports::bedrock_ecs::plugin::metadata::{Guest, Manifest},
};

mod bindings {
    wit_bindgen::generate!({
        path: ["./wit"],
        world: "component:plugin/plugin",
        with: {
            "bedrock-ecs:plugin/server": generate,
            "bedrock-ecs:plugin/metadata": generate,
        }
    });

    use super::Plugin;
    export!(Plugin);
}

struct Plugin;

impl Guest for Plugin {
    fn get_manifest() -> Manifest {
        let version = server::get_version();
        println!("server version is: {version:?}");

        Manifest {
            name: String::from("plugin"),
            version: vec![0, 1, 0],
        }
    }
}
