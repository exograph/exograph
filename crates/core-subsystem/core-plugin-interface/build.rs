// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

fn main() {
    // generates information for the `built` crate, which provides build-time information
    // to code using the current crate
    //
    // we use `built` primarily in crate::build_info::interface_version, where the information
    // is used to ensure compatibility between exo-server and a dynamic library
    built::write_built_file().expect("Failed to acquire build-time information");
}
