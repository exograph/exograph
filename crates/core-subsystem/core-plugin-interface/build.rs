fn main() {
    // generates information for the `built` crate, which provides build-time information
    // to code using the current crate
    //
    // we use `built` primarily in crate::build_info::interface_version, where the information
    // is used to ensure compatibility between clay-server and a dynamic library
    built::write_built_file().expect("Failed to acquire build-time information");
}
