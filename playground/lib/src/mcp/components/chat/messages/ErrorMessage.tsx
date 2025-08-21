// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { memo } from "react";

interface ErrorMessageProps {
  error: string;
}

export const ErrorMessage = memo(function ErrorMessage({ error }: ErrorMessageProps) {
  return (
    <div
      className="flex justify-start mb-6"
      role="alert"
      aria-label="Error message"
    >
      <div className="flex max-w-[80%]">
        <div
          className="flex-shrink-0 w-8 h-8 rounded-full bg-red-500 text-white mr-3 flex items-center justify-center text-sm font-medium"
          aria-hidden="true"
        >
          !
        </div>
        <div className="bg-red-50 dark:bg-red-900/30 border border-red-200 dark:border-red-800 px-4 py-3 rounded-2xl rounded-bl-md">
          <div className="flex items-center space-x-2">
            <div className="w-2 h-2 bg-red-500 rounded-full flex-shrink-0"></div>
            <span className="text-sm text-red-700 dark:text-red-300 break-words">
              {error}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
});