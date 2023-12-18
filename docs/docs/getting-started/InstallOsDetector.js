import { useLayoutEffect } from 'react';

export default function InstallOsDetector() {
  useLayoutEffect(() => {
    // For the use in setting the right tab for the install instructions. See ./index.md
    let userAgent = navigator.userAgent;
    if (userAgent.indexOf("Windows") > -1) {
      localStorage.setItem("docusaurus.tab.install-os", "windows");
    } else if (userAgent.indexOf("Mac") > -1 || userAgent.indexOf("Linux") > -1) {
      localStorage.setItem("docusaurus.tab.install-os", "mac-linux");
    }
  }, []);
}