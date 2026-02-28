use crate::{local::Local, system::Systems};

// fn system1() {

// }

fn system2(mut counter: Local<usize>) {
    // *counter += 1;
}

#[test]
fn system_test() {
    let mut systems = Systems::new();

    // systems.push(system1);
    systems.push(system2);

    todo!();
}