import React, { ButtonHTMLAttributes } from "react";
import { Store, iNotification } from "react-notifications-component";
import { Link, LinkProps, useLocation } from "react-router-dom";

export function Button({
  children,
  theme: color,
  className,
  onClick,
  ...rest
}: {
  children: React.ReactNode;
  theme?: ThemeColor;
  className?: string;
} & ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <SmallButton
      theme={color}
      className={(className ?? "") + " px-4 py-2"}
      {...rest}
    >
      {children}
    </SmallButton>
  );
}

export function SmallButton({
  children,
  theme,
  className,
  ...rest
}: {
  children: React.ReactNode;
  theme?: ThemeColor;
  className?: string;
} & ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      className={`transition-colors
			${themeColors(theme ?? "primary")}
			focus:outline-none focus:ring
			text-white rounded-md ${className ?? ""}`}
      {...rest}
    >
      {children}
    </button>
  );
}

export type ThemeColor =
  | "primary"
  | "success"
  | "danger"
  | "warning"
  | "info"
  | "gray";

export function themeColors(theme: ThemeColor): string {
  let className;

  switch (theme ?? "primary") {
    case "primary":
      className =
        "bg-blue-500 hover:bg-blue-600 hover:clicked:bg-blue-100 active:bg-blue-800 focus:ring-blue-300";
      break;

    case "success":
      className =
        "bg-green-600 hover:bg-green-700 hover:clicked:bg-green-200 active:bg-green-800 focus:ring-green-400";
      break;

    case "warning":
      className =
        "bg-yellow-500 hover:bg-yellow-600 hover:clicked:bg-yellow-100 active:bg-yellow-800 focus:ring-yellow-300";
      break;

    case "danger":
      className =
        "bg-red-500 hover:bg-red-600 hover:clicked:bg-red-100 active:bg-red-800 focus:ring-red-300";
      break;

    case "info":
      className =
        "bg-indigo-500 hover:bg-indigo-600 hover:clicked:bg-indigo-100 active:bg-indigo-800 focus:ring-indigo-300";
      break;

    default:
      className =
        "bg-gray-500 hover:bg-gray-600 hover:clicked:bg-gray-100 active:bg-gray-800 focus:ring-blue-300";
  }

  return className;
}

export function NavLabel(props: {
  children?: React.ReactNode;
  highlightMatch: string;
}) {
  const location = useLocation();
  const isStartWith = location.pathname.startsWith(props.highlightMatch);
  const className = "hover:scale-110 " + (isStartWith ? "font-bold" : "");

  return <div className={className}>{props.children}</div>;
}

export function Spinner(prop: {
  className?: string;
  style?: "ping" | "ring" | "flower" | "arrow";
}) {
  const cn = prop.className ?? "";
  const Inner = () => {
    switch (prop.style ?? "ring") {
      case "ring":
        return <i className={"ri-loader-4-line animate-spin " + cn} />;

      case "flower":
        return <i className={"ri-loader-line animate-spin " + cn} />;

      case "arrow":
        return <i className={"ri-loop-right-line animate-spin " + cn} />;

      case "ping":
        return (
          <div className="relative flex self-center">
            <i
              className={
                "absolute inline-flex w-full h-full ri-checkbox-blank-circle-fill animate-ping " +
                cn
              }
            />
            <i
              className={
                "relative inline-flex w-full h-full ri-checkbox-blank-circle-fill " +
                cn
              }
            />
          </div>
        );
    }
  };

  return (
    <div className="flex justify-center">
      <Inner />
    </div>
  );
}

export function LinkLabelRelPath(prop: {
  to: string;
  highlight: boolean;
  children: React.ReactNode;
}) {
  return (
    <>
      <Link to={prop.to} relative="path">
        <div className={"hover:italic " + (prop.highlight ? "font-bold" : "")}>
          {prop.children}
        </div>
      </Link>
    </>
  );
}
