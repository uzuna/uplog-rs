import React, { KeyboardEvent, useEffect, useRef, useState } from 'react'
import { Routes, Route, Link, useParams } from 'react-router-dom'
import Container from '@material-ui/core/Container'
import { withStyles } from '@material-ui/core/styles'
import Table from '@material-ui/core/Table'
import TableBody from '@material-ui/core/TableBody'
import TableCell from '@material-ui/core/TableCell'
import TableHead from '@material-ui/core/TableHead'
import TableRow from '@material-ui/core/TableRow'
import Button from '@material-ui/core/Button'

import { GraphQLClient } from 'graphql-request'
import {
  getSdk,
  SessionViewInfo,
  LogRecord,
  GetRecordsQueryVariables,
} from './generated/generated'
import { AppBar, Grid, Toolbar, Typography } from '@material-ui/core'

const client = new GraphQLClient('http://localhost:8040/graphql')
const sdk = getSdk(client)

class DataManager {
  private data: LogRecord[] = []
  private data_filtered: LogRecord[] = []
  private max_len: number = 10000
  private regexp: RegExp | null = null

  setRegexp(r: RegExp | null) {
    this.regexp = r
    this.dataFilter()
  }

  setData(data: LogRecord[]) {
    this.data = data
    this.dataFilter()
  }

  addData(data: LogRecord[]) {
    this.data = this.data.concat(data)
    this.data.sort((a, b) => a.id - b.id)
    this.dataFilter()
  }

  private dataFilter() {
    if (this.regexp != null) {
      let data = this.data.filter((x) => {
        return (
          this.regexp?.test(x.record.category) ||
          this.regexp?.test(x.record.message)
        )
      })
      this.data_filtered = data.slice(-this.max_len)
    } else {
      this.data_filtered = this.data.slice(-this.max_len)
    }
  }

  actualData(): LogRecord[] {
    return this.data_filtered
  }

  latestId(): number {
    return this.data[this.data.length - 1].id
  }
}

const CustomTableCell = withStyles((theme) => ({
  head: {
    backgroundColor: theme.palette.common.black,
    color: theme.palette.common.white,
  },
  body: {
    padding: 1,
  },
}))(TableCell)

interface StorageState {
  storages: SessionViewInfo[]
}

class Storages extends React.Component<{}, StorageState> {
  public state: StorageState = { storages: [] }
  async loadStorages() {
    const response = await sdk.getStorages()
    const req = { storages: response.storages }
    this.setState(req)
  }
  async componentDidMount() {
    await this.loadStorages()
  }

  render() {
    return (
      <div className="Storages">
        <Table>
          <TableHead>
            <TableRow>
              <CustomTableCell>created at</CustomTableCell>
              <CustomTableCell>name</CustomTableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {this.state.storages.map((item: SessionViewInfo) => (
              <TableRow>
                <CustomTableCell>{item.createdAt}</CustomTableCell>
                <CustomTableCell>
                  <Link to={'/session/' + item.name}>{item.name}</Link>
                </CustomTableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    )
  }
}

type SessionProps = { sessionId: string }

function Session() {
  const tableBottom = useRef<HTMLHeadingElement>(null)
  const executeScroll = () => tableBottom.current?.scrollIntoView()

  let [stateData, setData] = useState<LogRecord[]>()
  let params = useParams<SessionProps>()
  let db = useRef(new DataManager())
  const fetchLength = 1000

  useEffect(() => {
    const f = async () => {
      if (params.sessionId === undefined) {
        return
      }
      const req: GetRecordsQueryVariables = {
        name: params.sessionId,
        start: 0,
        length: fetchLength,
      }
      const response = await sdk.getRecords(req)
      db.current?.setData(response.storageReadAt)
      setData(db.current?.actualData())
    }
    f()
  }, [])

  const fetchNext = () => {
    let f = async () => {
      if (params.sessionId === undefined) {
        return
      }
      const req: GetRecordsQueryVariables = {
        name: params.sessionId,
        start: db.current?.latestId() + 1,
        length: fetchLength,
      }
      const response = await sdk.getRecords(req)
      db.current?.addData(response.storageReadAt)
      setData(db.current?.actualData())
      executeScroll()
    }
    f()
  }
  const filterKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      if (e.currentTarget.value.length > 0) {
        db.current?.setRegexp(new RegExp(e.currentTarget.value))
        setData(db.current?.actualData())
      }
    }
  }

  return (
    <div className="Session">
      <div>
        <Button variant="outlined" onClick={executeScroll}>
          Jump to latest
        </Button>
        <input
          type="text"
          title="regexp filter"
          // onChange={(e) => setFilterText(e.target.value)}
          onKeyDown={filterKeyDown}
        />
      </div>
      <Table>
        <TableHead>
          <TableRow>
            <CustomTableCell>rid</CustomTableCell>
            <CustomTableCell>level</CustomTableCell>
            <CustomTableCell>elapsed</CustomTableCell>
            <CustomTableCell>category</CustomTableCell>
            <CustomTableCell>message</CustomTableCell>
            <CustomTableCell>kv json</CustomTableCell>
          </TableRow>
        </TableHead>
        {stateData?.map((item: LogRecord) => (
          <TableBody>
            <TableRow>
              <CustomTableCell>{item.id}</CustomTableCell>
              <CustomTableCell>{item.record.level}</CustomTableCell>
              <CustomTableCell>
                {Math.round(item.record.elapsed * 1000) / 1000}
              </CustomTableCell>
              <CustomTableCell>{item.record.category}</CustomTableCell>
              <CustomTableCell>{item.record.message}</CustomTableCell>
              <CustomTableCell>{item.record.kv?.json}</CustomTableCell>
            </TableRow>
          </TableBody>
        ))}
      </Table>
      <div ref={tableBottom}></div>
      <Button variant="outlined" onClick={fetchNext}>
        fetch next
      </Button>
    </div>
  )
}

function AppHeader() {
  return (
    <AppBar position="static">
      <Container maxWidth="xl">
        <Toolbar>
          <Grid container spacing={2}>
            <Grid item xs={1}>
              <Typography>Uplog</Typography>
            </Grid>

            <Grid item xs={1}>
              <Typography>
                <Link style={{ textDecoration: 'none' }} to="/storages">
                  Storages
                </Link>
              </Typography>
            </Grid>
          </Grid>
        </Toolbar>
      </Container>
    </AppBar>
  )
}

function App() {
  return (
    <div>
      <AppHeader />
      <Container maxWidth="xl">
        <Routes>
          <Route path="/storages" element={<Storages />} />
          <Route path="/session/:sessionId" element={<Session />} />
        </Routes>
      </Container>
    </div>
  )
}

export default App
