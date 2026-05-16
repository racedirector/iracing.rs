import { useEffect, useState } from "react";
import { NavigationContainer } from "@react-navigation/native";
import { createMaterialTopTabNavigator } from "@react-navigation/material-top-tabs";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Header } from "./components/Header";
import {
  CONNECTION_STATE_CHANGED_EVENT,
  disconnectedConnectionState,
  initialConnectionState,
  type IRacingConnectionState,
} from "./connection";
import { IbtScreen } from "./screens/IbtScreen";
import { LiveScreen } from "./screens/LiveScreen";
import "./App.css";

type RootTabParamList = {
  Live: undefined;
  Ibt: undefined;
};

const Tab = createMaterialTopTabNavigator<RootTabParamList>();

function App() {
  const [connectionState, setConnectionState] = useState<IRacingConnectionState>(
    initialConnectionState,
  );

  useEffect(() => {
    let isMounted = true;
    let shouldUnlisten = false;
    let unlisten: (() => void) | undefined;

    async function observeConnectionState() {
      try {
        unlisten = await listen<IRacingConnectionState>(
          CONNECTION_STATE_CHANGED_EVENT,
          (event) => {
            if (isMounted) {
              setConnectionState(event.payload);
            }
          },
        );

        if (shouldUnlisten) {
          unlisten();
          return;
        }

        const initialState = await invoke<IRacingConnectionState>(
          "observe_connection_state",
        );

        if (isMounted) {
          setConnectionState(initialState);
        }
      } catch {
        if (isMounted) {
          setConnectionState(disconnectedConnectionState);
        }
      }
    }

    observeConnectionState();

    return () => {
      isMounted = false;
      shouldUnlisten = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  return (
    <div className="app-shell">
      <Header connectionState={connectionState} />

      <main
        className="app-content"
        aria-label="iRacing status workspace"
      >
        <NavigationContainer>
          <Tab.Navigator
            initialRouteName="Live"
            screenOptions={{
              tabBarActiveTintColor: "#17202a",
              tabBarInactiveTintColor: "#607085",
              tabBarIndicatorStyle: {
                backgroundColor: "#3074d4",
                height: 3,
              },
              tabBarLabelStyle: {
                fontSize: 13,
                fontWeight: "800",
                textTransform: "uppercase",
              },
              tabBarStyle: {
                backgroundColor: "#ffffff",
                borderBottomColor: "#d8dee6",
                borderBottomWidth: 1,
                elevation: 0,
                shadowOpacity: 0,
              },
            }}
          >
            <Tab.Screen
              name="Live"
              component={LiveScreen}
              options={{ title: "Live" }}
            />
            <Tab.Screen
              name="Ibt"
              component={IbtScreen}
              options={{ title: "IBT" }}
            />
          </Tab.Navigator>
        </NavigationContainer>
      </main>
    </div>
  );
}

export default App;
