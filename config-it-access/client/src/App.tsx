import React, { useEffect, useState } from "react";
import { BrowserRouter as Router, Routes, Route } from "react-router-dom";
import { Link } from "react-router-dom";
import { ReactNotifications, Store } from "react-notifications-component";
import "react-notifications-component/dist/theme.css";
import "remixicon/fonts/remixicon.css";
import { About, Dashboard, PrettyConfigItAccessText, RepoIcon } from "./Home";
import { NavLabel } from "./Widgets";
import {
  LoginPage,
  LoginSessInfo,
  NavLoginWidget,
  tryRestoreLoginSession,
} from "./LoginPage";

import dayjs from "dayjs";
import relativeTime from "dayjs/plugin/relativeTime";
import { Management } from "./Management";
dayjs.extend(relativeTime);

export const AuthContext = React.createContext({
  login: null as null | LoginSessInfo,
  setLogin: {} as (x: null | LoginSessInfo) => void,
});

export const SessExpireContext = React.createContext({
  value: null as null | bigint,
  setValue: {} as (x: null | bigint) => void,
});

function App() {
  const [login, setLogin] = React.useState(null as null | LoginSessInfo);
  const sessExpireRef = React.useRef(null as null | bigint);
  const [sessionRestorationAttempted, setSessionRestorationAttempted] =
    useState(false);

  useEffect(() => {
    if (!sessionRestorationAttempted) {
      tryRestoreLoginSession(setLogin, (val) => {
        sessExpireRef.current = val;
      }).finally(() => {
        setSessionRestorationAttempted(true);
      });
    }
  }, []);

  function setLoginState(newLogin: null | LoginSessInfo) {
    setLogin((prev) => {
      if (prev !== null && newLogin === null) {
        // TODO: Push logout notification
        Store.addNotification({
          container: "bottom-center",
          dismiss: { duration: 1500 },
          type: "info",
          title: "Logged out",
        });
      }
      return newLogin;
    });
  }

  if (!sessionRestorationAttempted) {
    return (
      <>
        <div className="flex flex-col h-screen">
          <ReactNotifications />
          <div className="flex-grow overflow-y-auto">
            <div className="flex flex-col items-center justify-center h-full">
              <div className="flex flex-col items-center justify-center text-8xl">
                <PrettyConfigItAccessText />
                <div className="text-4xl mt-6 text-gray-300">
                  Restoring previous session ..
                </div>
              </div>
            </div>
          </div>
        </div>
      </>
    );
  }

  return (
    <Router>
      <div className="app-container flex flex-col h-screen">
        <ReactNotifications />
        <SessExpireContext.Provider
          value={{
            value: sessExpireRef.current,
            setValue: (val) => {
              sessExpireRef.current = val;
            },
          }}
        >
          <AuthContext.Provider
            value={{
              login: login,
              setLogin: setLoginState,
            }}
          >
            <NavBar login={login} />
            <div className="flex-grow overflow-y-auto">
              <Routes>
                {
                  <Route
                    path="/"
                    element={login ? <Dashboard /> : <LoginPage />}
                  />
                }
                <Route path="/about" element={<About />} />
                {login && <Route path="/sites" element={<Sites />} />}
                {/* TODO: Individual session route () */}
                <Route path="/management/:page" element={<Management />} />
                {login && <Route path="/account" element={<Account />} />}
                <Route path="/login" element={<LoginPage />} />
                <Route path="*" element={<>404 NOT FOUND</>} />
              </Routes>
            </div>
          </AuthContext.Provider>
        </SessExpireContext.Provider>
      </div>
    </Router>
  );
}

function NavBar(prop: { login: null | LoginSessInfo }) {
  const { login } = prop;

  return (
    <>
      <nav className="flex p-2 px-3 items-center bg-blue-500 text-white">
        <Link
          to="/"
          className="scale-110 font-extrabold ml-4 mr-8 hover:scale-125 transition-transform"
        >
          <PrettyConfigItAccessText />
        </Link>
        {login && (
          <Link to="/sites" className="mr-4">
            <NavLabel highlightMatch="/sites">Sites</NavLabel>
          </Link>
        )}
        <Link to="/management/users" className="mr-4">
          <NavLabel highlightMatch="/management">Management</NavLabel>
        </Link>
        <Link to="/about" className="mr-4">
          <NavLabel highlightMatch="/about">About</NavLabel>
        </Link>
        <div className="flex-auto" />
        <NavLoginWidget />
        <RepoIcon className="ml-5 fill-white" />
      </nav>
    </>
  );
}

export default App;

function Sites() {
  // TODO:

  return <div>Sessions Page Content</div>;
}

function Account() {
  return <div>Account Page Content</div>;
}

export async function getSHA256Hash(
  input: string,
  asHexString: boolean = false
) {
  const textAsBuffer = new TextEncoder().encode(input);
  const hashBuffer = await window.crypto.subtle.digest("SHA-256", textAsBuffer);
  const hashArray = Array.from(new Uint8Array(hashBuffer));

  if (asHexString) {
    return hashArray.map((b) => b.toString(16).padStart(2, "0")).join("");
  }

  return hashArray;
}
