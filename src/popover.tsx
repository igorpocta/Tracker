import ReactDOM from "react-dom/client";
import "./index.css";

function Popover() {
  return <div className="p-4 text-white">Popover</div>;
}

ReactDOM.createRoot(document.getElementById("root")!).render(<Popover />);
