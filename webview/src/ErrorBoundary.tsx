// A page that crashes must say why, not turn into a blank tab: the error text is what a bug report needs.

import { Component, type ErrorInfo, type ReactNode } from "react";

import { post } from "./host.ts";

export class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    post({ type: "error", message: `${error.message}\n${info.componentStack ?? ""}` }); // the host logs it: a blank or broken tab must not go unnoticed
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div className="p-4 text-[13px]">
        <p className="font-semibold text-destructive">V-NOC hit an error drawing this tab.</p>
        <pre className="mt-2 whitespace-pre-wrap text-muted-foreground">{error.stack ?? error.message}</pre>
        <p className="mt-2 text-muted-foreground">Close the tab and open it again. If it keeps happening, copy the text above into a bug report.</p>
      </div>
    );
  }
}
