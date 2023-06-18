import React, { useContext } from 'react';
import { BrowserRouter as Router, Routes, Route } from 'react-router-dom';
import { Link } from 'react-router-dom';
import { ReactNotifications } from 'react-notifications-component'
import 'react-notifications-component/dist/theme.css'
import { Home } from './Home';

const AuthContext = React.createContext({
  isLogin: false,
  setIsLogin: {} as (x: boolean) => void,
  isMgmtVisible: false,
  setIsMgmtVisible: {} as (x: boolean) => void,
});

function App() {
  const [isLogin, setIsLogin] = React.useState(false);
  const [isMgmtVisible, setIsMgmtVisible] = React.useState(false);

  return (
    <Router>
      <div className='app-container flex flex-col h-screen'>
        <AuthContext.Provider value={{ isLogin, setIsLogin, isMgmtVisible, setIsMgmtVisible }}>
          <ReactNotifications />

          <nav className="flex p-4 bg-blue-500 text-white">
            <Link to="/" className='font-extrabold mr-8 text-yellow-200'>Config-it-Access</Link>
            <Link to="/sessions" className="mr-4">Sessions</Link>
            {isLogin && <Link to="/chat" className="mr-4">Chat</Link>}
            {isMgmtVisible && <Link to="/management" className="mr-4">Management</Link>}
            <Link to="/account" className="mr-4">Account</Link>
            <div className='flex-auto' />
            <LoginLogoutBtn />
          </nav>

          <div className='flex-grow overflow-y-auto'>
            <Routes>
              <Route path="/" element={<Home />} />
              <Route path="/sessions" element={<Sessions />} />
              {/* TODO: Individual session route () */}
              <Route path="/chat" element={<Chat />} />
              {isMgmtVisible && <Route path="/management" element={<Management />} />}
              <Route path="/account" element={<Account />} />
              <Route path="/login" element={<Account />} />
            </Routes>
          </div>

        </AuthContext.Provider>
      </div></Router>
  );
}

function LoginLogoutBtn() {
  const { isLogin, setIsLogin } = useContext(AuthContext);

  return <>
    <button
      className=""
      onClick={() => setIsLogin(!isLogin)}>
      {isLogin ? "Logout" : "Login"}
    </button>
  </>
}

function LoginPage() {
  return <>
    Login Page Content Here!
  </>
}

export default App;

function Sessions() {
  // TODO: 

  return <div>Sessions Page Content</div>;
}

function Chat() {
  return <div>Chat Page Content</div>;
}

function Management() {
  return <div>Management Page Content</div>;
}

function Account() {
  return <div>Account Page Content</div>;
}


