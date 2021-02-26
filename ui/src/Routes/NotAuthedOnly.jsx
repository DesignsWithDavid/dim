import React, { useEffect } from "react";
import { Route, useHistory } from "react-router-dom";
import { connect } from "react-redux";

import { updateAuthToken } from "../actions/auth.js";

function NotAuthedOnlyRoute(props) {
  const history = useHistory();
  const tokenInCookie = document.cookie.split("=")[1];

  const { updateAuthToken, auth } = props;
  const { logged_in, error } = auth.login;
  const { token } = auth;

  useEffect(() => {
    if (tokenInCookie) {
      updateAuthToken(tokenInCookie);
    }

		if (logged_in && token && !error && !tokenInCookie) {
      const dateExpires = new Date();

      dateExpires.setTime(dateExpires.getTime() + 604800000);

      document.cookie = (
        `token=${token};expires=${dateExpires.toGMTString()};`
      );

      history.push("/")
    }

    if (token && tokenInCookie) {
      history.push("/");
    }
  }, [error, history, logged_in, token, tokenInCookie, updateAuthToken]);

  // auto login when logged in another instance
  useEffect(() => {
    const bc = new BroadcastChannel("dim");

    bc.onmessage = (e) => {
      if (document.hasFocus()) return;

      if (e.data === "login") {
        /*
          cannot use history.push, throws an error when
          tab is not active and it tries to redirect.
        */
        window.location.replace("/");
      }
    };

    return () => bc.close();
  }, []);

  /*
    scroll to top on route change
    (cannot enable smooth -> page doesn't go fully up)
  */
  useEffect(() => {
    window.scrollTo(0, 0);
  }, [history.location.pathname]);

  const { exact, path, render, children } = props;

  return (!token && !tokenInCookie) && (
    <Route exact={exact} path={path} render={render} children={children}/>
  );
}

const mapStateToProps = (state) => ({
  auth: state.auth
});

const mapActionsToProps = ({
  updateAuthToken
});

export default connect(mapStateToProps, mapActionsToProps)(NotAuthedOnlyRoute);