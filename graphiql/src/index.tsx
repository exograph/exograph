// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { createRoot } from "react-dom/client";
import "./index.css";
import { AppWithAuth } from "./App";

const container = document.getElementById("root");
const root = createRoot(container as HTMLElement);
root.render(<AppWithAuth />);
