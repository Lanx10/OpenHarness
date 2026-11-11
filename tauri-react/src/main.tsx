import { createRoot } from 'react-dom/client';
import './components/tokens.css';
import './components/base.css';
import './style.css';
import {OpenHarness} from './OpenHarness';

createRoot(document.querySelector('#root')!).render(<OpenHarness />);
