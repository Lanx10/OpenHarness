import { createRoot } from 'react-dom/client';
import './style.css';
import {OpenHarness} from './OpenHarness';

createRoot(document.querySelector('#root')!).render(<OpenHarness />);
