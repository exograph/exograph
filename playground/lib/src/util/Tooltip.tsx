// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import React, { useState, useRef, useEffect } from "react";

interface TooltipProps {
  content: string;
  children: React.ReactNode;
  className?: string;
  position?: "top" | "bottom" | "left" | "right";
  size?: "sm" | "md" | "lg";
}

export function Tooltip({
  content,
  children,
  className = "",
  position = "bottom",
  size = "md",
}: TooltipProps) {
  const [showTooltip, setShowTooltip] = useState(false);
  const triggerRef = useRef<HTMLDivElement>(null);

  // Close tooltip when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        triggerRef.current &&
        !triggerRef.current.contains(event.target as Node)
      ) {
        setShowTooltip(false);
      }
    };

    if (showTooltip) {
      document.addEventListener("mousedown", handleClickOutside);
      return () =>
        document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [showTooltip]);

  const sizeClasses = {
    sm: "px-2 py-1 text-xs w-48",
    md: "px-3 py-2 text-sm w-64",
    lg: "px-4 py-3 text-sm w-72",
  };

  const positionClasses = {
    top: "bottom-full mb-2",
    bottom: "top-full mt-2",
    left: "right-full mr-2 top-1/2 transform -translate-y-1/2",
    right: "left-full ml-2 top-1/2 transform -translate-y-1/2",
  };

  const arrowClasses = {
    top: "top-full left-1/2 transform -translate-x-1/2 border-t-black border-l-transparent border-r-transparent border-b-transparent",
    bottom:
      "-top-1 left-1/2 transform -translate-x-1/2 border-b-black border-l-transparent border-r-transparent border-t-transparent rotate-45",
    left: "left-full top-1/2 transform -translate-y-1/2 border-l-black border-t-transparent border-b-transparent border-r-transparent",
    right:
      "right-full top-1/2 transform -translate-y-1/2 border-r-black border-t-transparent border-b-transparent border-l-transparent",
  };

  const alignmentClasses = {
    top: "left-1/2 transform -translate-x-1/2",
    bottom: "left-1/2 transform -translate-x-1/2",
    left: "",
    right: "",
  };

  return (
    <div className={`relative inline-block ${className}`} ref={triggerRef}>
      <div onClick={() => setShowTooltip(!showTooltip)}>{children}</div>

      {showTooltip && (
        <div
          className={`absolute z-50 ${sizeClasses[size]} ${positionClasses[position]} ${alignmentClasses[position]} bg-black text-white rounded-lg shadow-2xl border border-gray-600`}
        >
          <div
            className={`absolute w-2 h-2 bg-black border-l border-t border-gray-600 ${arrowClasses[position]}`}
          ></div>
          {content}
        </div>
      )}
    </div>
  );
}
