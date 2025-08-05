// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { AuthToolbarButton } from "./auth";
import { Logo } from "./Logo";

export function Navbar() {
  return (
    <nav className="flex items-center justify-between px-3 py-2 border-b bg-white border-gray-200 dark:bg-gray-800 dark:border-gray-700 h-12">
      <Logo />
      <AuthToolbarButton />
    </nav>
  );
}
