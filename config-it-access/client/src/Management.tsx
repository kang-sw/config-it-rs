import React from "react";
import {
  BrowserRouter,
  Link,
  Route,
  Routes,
  useLocation,
  useParams,
  useResolvedPath,
} from "react-router-dom";
import { LinkLabelRelPath, NavLabel } from "./Widgets";

export function Management() {
  // retrieve relative part of path
  const prefix = useResolvedPath(".");
  const { pathname } = useLocation();
  const page = pathname.slice(prefix.pathname.length + 1).toLowerCase();

  function NavNode(prop: { name: string }) {
    const lower = prop.name.toLowerCase();
    return (
      <LinkLabelRelPath to={"../" + lower} highlight={page.startsWith(lower)}>
        {prop.name}
      </LinkLabelRelPath>
    );
  }

  return (
    <div className="flex flex-row h-full">
      <div className="grow-0 w-44 flex flex-col px-3">
        <NavNode name="Account" />
        <NavNode name="Users" />
        <NavNode name="Roles" />
        <NavNode name="Sites" />
        <NavNode name="ApiKeys" />
        <NavNode name="System" />
      </div>
      <div className="bg-black w-px mx-4 my-2" />{" "}
      <div className="flex-grow">
        <Routes>
          <Route path="account" element={<ThisAccount />} />
          <Route path="users" element={<>TODO</>} />
          <Route path="roles" element={<>TODO</>} />
          <Route path="sites" element={<>TODO</>} />
          <Route path="apikeys" element={<>TODO</>} />
          <Route path="system" element={<>TODO</>} />
          <Route path="*" element={<DebugFallbackRoute />} />
        </Routes>
      </div>
    </div>
  );
}

function ThisAccount() {
  // TODO: Change password, notification methods, etc.
  return <>ACCOUNT INFO HERE</>;
}

function UserList() {
  return <></>;
}

function DebugFallbackRoute() {
  const location = useLocation();
  const params = useParams();

  return (
    <div>
      {location.pathname}, {JSON.stringify(params)}
    </div>
  );
}
