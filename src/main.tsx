import { createRoot } from 'react-dom/client'
import '@fontsource/dm-sans/latin-400.css'
import '@fontsource/dm-sans/latin-500.css'
import '@fontsource/dm-sans/latin-600.css'
import '@fontsource/dm-sans/latin-700.css'
import './styles.css'
import Capture from './Capture'
import Dashboard from './Dashboard'

const capture = new URLSearchParams(window.location.search).get('window') === 'capture'
document.body.classList.toggle('is-capture', capture)
createRoot(document.getElementById('root')!).render(capture ? <Capture /> : <Dashboard />)
