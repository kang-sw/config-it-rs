import React, { useContext, useEffect, useReducer } from "react";
import { PrettyConfigItAccessText } from "./Home";
import { Button, NavLabel, SmallButton, Spinner } from "./Widgets";
import { AuthContext, SessExpireContext, getSHA256Hash } from "./App";
import { Store } from "react-notifications-component";
import { Link, Navigate, useNavigate } from "react-router-dom";
import { useInterval } from "usehooks-ts";
import dayjs from "dayjs";
import { LoginReply } from "@bindings/LoginReply";

export interface LoginSessInfo {
  user_alias: string;
  user_id: string;
  authority: number;
}

export function LoginPage() {
  const { login, setLogin } = useContext(AuthContext);
  const { setValue: setSessExpire } = useContext(SessExpireContext);
  const [loggingIn, setLoggingIn] = React.useState(false);

  async function onSubmit(e: React.FormEvent<HTMLFormElement>) {
    setLoggingIn(true);
    e.preventDefault();
    const target = e.target as HTMLFormElement;
    const password = target.user_pw.value;

    const pwHash = await getSHA256Hash(password, true);
    const noti = Store.addNotification({
      container: "bottom-center",
      type: "default",
      title: (
        <div className="flex flex-row">
          <Spinner style="arrow" />
          <div className="self-center ml-2">Logging in...</div>
        </div>
      ),
      message: "Click to cancel",
      dismiss: { duration: 30_000_000, click: true },
    });

    try {
      const fetchFuture = fetch("/api/login", {
        method: "POST",
        headers: {
          Authorization: `Basic ${btoa(`${target.user_id.value}:${pwHash}`)}`,
          credentials: "include",
        },
      });

      const result = await fetchFuture;
      if (result.status === 200) {
        const object = (await result.json()) as LoginReply;
        target.user_pw.value = null;

        setSessExpire(object.expire_utc_ms);
        setLogin({
          user_id: target.user_id.value,
          user_alias: object.user_alias,
          authority: object.authority,
        });
      } else {
        Store.addNotification({
          container: "bottom-center",
          type: "danger",
          title: "Login failed",
          message: `Status: ${result.status} ${result.statusText}`,
          dismiss: { duration: 3000 },
        });
      }
    } catch (e) {
      Store.addNotification({
        container: "bottom-center",
        type: "danger",
        title: "Login failed - Exception",
        message: `${e}`,
        dismiss: { duration: 3000 },
      });
    } finally {
      setLoggingIn(false);
      Store.removeNotification(noti);
    }
  }

  if (login) {
    return <Navigate to="/" />;
  }

  const inputClass =
    "bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 disabled:bg-slate-200";

  return (
    <>
      <div className="flex flex-col items-center justify-center h-full">
        <h1 className="text-6xl mb-6 font-bold">
          <PrettyConfigItAccessText />
        </h1>
        {/* <h2 className="text-2xl mt-1 font-bold">Login</h2> */}
        <form onSubmit={onSubmit} className="flex flex-col">
          <div className="grid gap-6 mb-6 mt-12 md:grid-cols-3 flex-row">
            <div>
              <label
                htmlFor="user_id"
                className="block mb-2 text-sm font-medium text-gray-900"
              >
                ID
              </label>
              <input
                type="text"
                id="user_id"
                className={inputClass}
                placeholder="id"
                required
                disabled={loggingIn}
              />
            </div>
            <div>
              <label
                htmlFor="user_pw"
                className="block mb-2 text-sm font-medium text-gray-900"
              >
                Password
              </label>
              <input
                type="password"
                id="user_pw"
                className={inputClass}
                placeholder="password"
                required
                disabled={loggingIn}
              />
            </div>
            {!loggingIn && (
              <Button
                type="submit"
                theme="success"
                className={"font-extrabold hover:text-4xl focus:text-4xl"}
                disabled={loggingIn}
              >
                <div className="transition-all">Login</div>
              </Button>
            )}
          </div>
        </form>
      </div>
    </>
  );
}

export async function tryRestoreLoginSession(
  setLogin: (x: null | LoginSessInfo) => void,
  setExpiration: (x: null | bigint) => void
) {
  let notiId = Store.addNotification({
    container: "bottom-right",
    dismiss: { duration: 1000000000 },
    type: "info",
    title: "Restoring Session",
    message: "Please wait...",
  });

  try {
    const restored = (await (
      await fetch("/api/sess/restore", {
        method: "POST",
      })
    ).json()) as LoginReply;

    setLogin({
      user_alias: restored.user_alias,
      user_id: restored.user_id,
      authority: restored.authority,
    });
    setExpiration(restored.expire_utc_ms);

    Store.addNotification({
      container: "bottom-right",
      dismiss: { duration: 3000 },
      type: "success",
      title: "Previous session restored",
    });
  } catch (e: any) {
    Store.addNotification({
      container: "bottom-right",
      dismiss: { duration: 1000 },
      type: "warning",
      title: "Failed to restore session",
      message: e.toString(),
    });

    setLogin(null);
    setExpiration(null);
  } finally {
    Store.removeNotification(notiId);
  }
}

export function NavLoginWidget() {
  const { login, setLogin } = useContext(AuthContext);
  const { value: expire, setValue: setExpire } = useContext(SessExpireContext);
  const [refreshCount, refresh] = useReducer((x) => x + 1, 0);
  const navigate = useNavigate();

  useInterval(refresh, 120_000);

  let timeStr = "";
  if (expire) {
    timeStr = dayjs.unix(Number(expire) / 1000).fromNow(true);
  }

  useEffect(() => {
    const expireTime = dayjs.unix(Number(expire) / 1000);
    const now = dayjs();
    const remains = expireTime.diff(now, "second");

    if (0 < remains && remains < 300) {
      Store.addNotification({
        container: "bottom-right",
        title: "Session",
        message: "Session extended automatically",
        type: "info",
        dismiss: { duration: 1500 },
      });
      extendSession();
    }
  }, [refreshCount]);

  async function extendSession() {
    try {
      const retval = await fetch("/api/sess/extend", { method: "POST" });
      if (retval.status === 200) {
        const new_expire_due = (await retval.json()) as bigint;
        setExpire(new_expire_due);

        Store.addNotification({
          container: "bottom-right",
          title: "Session",
          message: "Session extended",
          type: "info",
          dismiss: { duration: 1500 },
        });
      } else {
        Store.addNotification({
          container: "bottom-right",
          title: "Session Error",
          message: "Failed to extend session",
          type: "danger",
          dismiss: { duration: 3000 },
        });
        setLogin(null);
      }
    } catch (e) {
      Store.addNotification({
        container: "bottom-right",
        title: "Session Error",
        message: "Failed to extend session",
        type: "danger",
        dismiss: { duration: 3000 },
      });
      setLogin(null);
      setExpire(null);
    }
  }

  async function logout() {
    setLogin(null);
    await fetch("/api/sess/logout", { method: "POST" });
    navigate("/login");
  }

  return (
    <>
      {login ? (
        <div className="flex flex-row">
          <div className="text-sm self-center me-2">
            <span className="text-xs">Welcome</span>,{" "}
            <b className="italic">{login.user_alias}</b>
          </div>

          <SmallButton
            theme="success"
            className="text-xs px-2 me-2 hover:scale-110"
            title="Click to extend login session"
            onClick={extendSession}
          >
            <span className="ri-timer-line "></span> {timeStr}
          </SmallButton>

          <SmallButton
            theme="danger"
            className="text-xs my-0 px-2 hover:scale-110 text-slate-900"
            onClick={logout}
          >
            Logout
            <i className="ri-logout-circle-line ms-1" />
          </SmallButton>
        </div>
      ) : (
        <Link to="/login">
          <NavLabel highlightMatch="/login">Login</NavLabel>
        </Link>
      )}
    </>
  );
}
