pub struct World {

}

impl World {
    pub fn new() -> World {
        World::default()
    }
}

impl Default for World {
    fn default() -> World {
        World {}
    }
}