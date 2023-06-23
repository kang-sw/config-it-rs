import React, { useContext } from "react";
import { PrettyConfigItAccessText } from "./Home";
import { Button, Spinner } from "./Widgets";
import { AuthContext, getSHA256Hash } from "./App";
import { Store } from "react-notifications-component";

export function LoginPage() {
  const { setLogin } = useContext(AuthContext);
  const [loggingIn, setLoggingIn] = React.useState(false);

  async function onSubmit(e: React.FormEvent<HTMLFormElement>) {
    setLoggingIn(true);
    e.preventDefault();
    const target = e.target as HTMLFormElement;
    const password = target.user_pw.value;
    target.user_pw.value = null;

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
      onRemoval: () => {
        setLoggingIn(false);
      },
    });

    const fetchFuture = fetch("/api/login", {
      method: "POST",
      headers: {
        Authorization: `Basic ${btoa(`${target.user_id.value}:${pwHash}`)}`,
      },
    });

    await new Promise((resolve) => setTimeout(resolve, 1000));
    const result = await fetchFuture;

    if (result.status === 200) {
    } else {
      Store.addNotification({
        container: "bottom-center",
        type: "danger",
        title: "Login failed",
        message: `Status: ${result.status}\n${await result.text()}`,
        dismiss: { duration: 2000 },
      });
      setLoggingIn(false);
      return;
    }

    Store.removeNotification(noti);
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
