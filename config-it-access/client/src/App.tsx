import React, { useContext, useState } from 'react';
import { BrowserRouter as Router, Routes, Route } from 'react-router-dom';
import { Link } from 'react-router-dom';
import { ReactNotifications } from 'react-notifications-component'
import 'react-notifications-component/dist/theme.css'
import 'remixicon/fonts/remixicon.css'
import { About, Dashboard, PrettyConfigItAccessText, RepoIcon } from './Home';
import { NavLabel, SmallButton } from './Widgets';
import { LoginPage } from './LoginPage';

export const AuthContext = React.createContext({
  login: null as null | string,
  setLogin: {} as (x: string | null) => void,
  isMgmtVisible: false,
  setIsMgmtVisible: {} as (x: boolean) => void,
});

function App() {
  const [login, setLogin] = React.useState(null as null | string);
  const [isMgmtVisible, setIsMgmtVisible] = React.useState(false);

  return (
    <Router>
      <div className='app-container flex flex-col h-screen'>
        <ReactNotifications />
        <AuthContext.Provider value={{ login: login, setLogin: setLogin, isMgmtVisible, setIsMgmtVisible }}>

          <NavBar login={login} isMgmtVisible={isMgmtVisible} />

          <div className='flex-grow overflow-y-auto'>
            <Routes>
              {login && <Route path="/" element={<Dashboard />} />}
              {!login && <Route path="/" element={<LoginPage />} />}
              <Route path="/about" element={<About />} />
              {login && <Route path="/sessions" element={<Sessions />} />}
              {/* TODO: Individual session route () */}
              {isMgmtVisible && <Route path="/management" element={<Management />} />}
              {login && <Route path="/account" element={<Account />} />}
              <Route path="/login" element={<LoginPage />} />
              <Route path="*" element={<>404 NOT FOUND</>} />
            </Routes>
          </div>

        </AuthContext.Provider>
      </div>
    </Router>
  );
}

function NavBar(prop: { login: null | string, isMgmtVisible: boolean }) {
  const { login, isMgmtVisible } = prop;

  return <>
    <nav className="flex p-4 bg-blue-500 text-white">
      <Link to="/"
        className='scale-110 font-extrabold ml-4 mr-8 hover:scale-125 transition-transform'>
        <PrettyConfigItAccessText />
      </Link>
      {login && <Link to="/sessions" className="mr-4">
        <NavLabel match='/sessions'>Sessions</NavLabel>
      </Link>}
      {isMgmtVisible && <Link to="/management" className="mr-4">
        <NavLabel match='/management'>Management</NavLabel>
      </Link>}
      {login && <Link to="/account" className="mr-4">
        <NavLabel match='/account'>Account</NavLabel>
      </Link>}
      <Link to="/about" className="mr-4">
        <NavLabel match='/about'>About</NavLabel>
      </Link>
      <div className='flex-auto' />
      {
        login
          ? <SmallButton>logout</SmallButton>
          : <Link to="/login"><NavLabel match='/login'>Login</NavLabel></Link>
      }
      <RepoIcon className='ml-5 fill-white' />
    </nav>
  </>
}


export default App;

function Sessions() {
  // TODO: 

  return <div>Sessions Page Content</div>;
}

function Management() {
  return <div>Management Page Content</div>;
}

function Account() {
  return <div>Account Page Content</div>;
}

export async function getSHA256Hash(input: string, asHexString: boolean = false) {
  const textAsBuffer = new TextEncoder().encode(input);
  const hashBuffer = await window.crypto.subtle.digest("SHA-256", textAsBuffer);
  const hashArray = Array.from(new Uint8Array(hashBuffer));

  if (asHexString) {
    return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
  }

  return hashArray;
};
