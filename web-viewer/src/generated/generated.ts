import { GraphQLClient } from 'graphql-request';
import * as Dom from 'graphql-request/dist/types.dom';
import gql from 'graphql-tag';
export type Maybe<T> = T | null;
export type InputMaybe<T> = Maybe<T>;
export type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
export type MakeOptional<T, K extends keyof T> = Omit<T, K> & { [SubKey in K]?: Maybe<T[SubKey]> };
export type MakeMaybe<T, K extends keyof T> = Omit<T, K> & { [SubKey in K]: Maybe<T[SubKey]> };
/** All built-in and custom scalars, mapped to their actual values */
export type Scalars = {
  ID: string;
  String: string;
  Boolean: boolean;
  Int: number;
  Float: number;
  DateTime: any;
  Duration: any;
};

export type KeyValue = {
  __typename?: 'KeyValue';
  json: Scalars['String'];
};

export enum LogLevel {
  Debug = 'DEBUG',
  Error = 'ERROR',
  Info = 'INFO',
  Trace = 'TRACE',
  Warn = 'WARN'
}

export type LogRecord = {
  __typename?: 'LogRecord';
  id: Scalars['Int'];
  record: RecordObject;
};

export type Query = {
  __typename?: 'Query';
  storageReadAt: Array<LogRecord>;
  storages: Array<SessionViewInfo>;
};


export type QueryStorageReadAtArgs = {
  vars: ReadAtVars;
};

export type ReadAtVars = {
  length?: InputMaybe<Scalars['Int']>;
  name: Scalars['String'];
  start?: InputMaybe<Scalars['Int']>;
};

export type RecordObject = {
  __typename?: 'RecordObject';
  category: Scalars['String'];
  elapsed: Scalars['Duration'];
  file?: Maybe<Scalars['String']>;
  kv?: Maybe<KeyValue>;
  level: LogLevel;
  line?: Maybe<Scalars['Int']>;
  message: Scalars['String'];
  modulePath?: Maybe<Scalars['String']>;
};

export type SessionViewInfo = {
  __typename?: 'SessionViewInfo';
  createdAt: Scalars['DateTime'];
  name: Scalars['String'];
  updatedAt: Scalars['DateTime'];
};

export type GetStoragesQueryVariables = Exact<{ [key: string]: never; }>;


export type GetStoragesQuery = { __typename?: 'Query', storages: Array<{ __typename?: 'SessionViewInfo', createdAt: any, updatedAt: any, name: string }> };

export type GetRecordsQueryVariables = Exact<{
  name: Scalars['String'];
  start: Scalars['Int'];
  length?: InputMaybe<Scalars['Int']>;
}>;


export type GetRecordsQuery = { __typename?: 'Query', storageReadAt: Array<{ __typename?: 'LogRecord', id: number, record: { __typename?: 'RecordObject', level: LogLevel, elapsed: any, category: string, message: string, modulePath?: string | null, line?: number | null, file?: string | null, kv?: { __typename?: 'KeyValue', json: string } | null } }> };


export const GetStoragesDocument = gql`
    query getStorages {
  storages {
    createdAt
    updatedAt
    name
  }
}
    `;
export const GetRecordsDocument = gql`
    query getRecords($name: String!, $start: Int!, $length: Int = 100) {
  storageReadAt(vars: {name: $name, start: $start, length: $length}) {
    id
    record {
      level
      elapsed
      category
      message
      modulePath
      line
      file
      kv {
        json
      }
    }
  }
}
    `;

export type SdkFunctionWrapper = <T>(action: (requestHeaders?:Record<string, string>) => Promise<T>, operationName: string) => Promise<T>;


const defaultWrapper: SdkFunctionWrapper = (action, _operationName) => action();

export function getSdk(client: GraphQLClient, withWrapper: SdkFunctionWrapper = defaultWrapper) {
  return {
    getStorages(variables?: GetStoragesQueryVariables, requestHeaders?: Dom.RequestInit["headers"]): Promise<GetStoragesQuery> {
      return withWrapper((wrappedRequestHeaders) => client.request<GetStoragesQuery>(GetStoragesDocument, variables, {...requestHeaders, ...wrappedRequestHeaders}), 'getStorages');
    },
    getRecords(variables: GetRecordsQueryVariables, requestHeaders?: Dom.RequestInit["headers"]): Promise<GetRecordsQuery> {
      return withWrapper((wrappedRequestHeaders) => client.request<GetRecordsQuery>(GetRecordsDocument, variables, {...requestHeaders, ...wrappedRequestHeaders}), 'getRecords');
    }
  };
}
export type Sdk = ReturnType<typeof getSdk>;